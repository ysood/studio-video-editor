//! Sign in with ChatGPT: authorization code with PKCE, and dynamic registration.
//!
//! Studio is a locally installed app, so it uses the route meant for those: it has no
//! client secret and no partner key, and registers itself the first time somebody signs
//! in. OpenAI issues a client id in the callback, which is kept and reused, so a second
//! sign-in does not register a second app in the person's ChatGPT settings.
//!
//! Everything that can be decided without the network is a plain function over its
//! input: deriving the PKCE challenge, checking the discovery document, building the
//! authorize URL, reading a token response. The network parts are the thin layer above
//! them. That split is what makes this testable at all, because nothing here can be
//! tested against the real provider from a test run.
//!
//! Three checks are load-bearing and easy to leave out:
//!
//! - **The endpoints are discovered, then verified.** They are read from the provider's
//!   OpenID configuration rather than written down here, and every one is then required
//!   to live on the issuer's own origin. A tampered configuration that pointed the token
//!   endpoint somewhere else would otherwise be handed an authorization code.
//! - **Redirects are not followed.** A redirect on a token request would resend the body,
//!   and the body holds the authorization code or the refresh token.
//! - **The ID token is verified, not read.** Signature against the provider's keys,
//!   issuer, audience, and the nonce from this attempt. An unverified `sub` is an
//!   attacker's choice of account.

use std::sync::OnceLock;
use std::time::Duration;

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use url::Url;
use uuid::Uuid;

use super::error::{ChatError, ChatErrorCode, ChatResult};

/// The provider. Every discovered endpoint is required to be on this origin.
pub const ISSUER: &str = "https://auth.openai.com";

/// What the tokens are for. Sent on every authorize, exchange and refresh, because the
/// grant is scoped to it.
pub const RESOURCE: &str = "https://api.openai.com/v1";

/// The name a person will see in their ChatGPT settings, where they can revoke this.
const AGENT_NAME: &str = "Studio";

/// The client id used only until a real one is issued. Never stored, never exchanged.
const FIRST_SIGN_IN: &str = "dynamic_agent_client";

const TIMEOUT: Duration = Duration::from_secs(20);

/// Where the provider says its endpoints are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovery {
    pub authorize: String,
    pub token: String,
    pub jwks: String,
    /// Absent for a provider that does not offer revocation, which makes signing out a
    /// local-only operation.
    pub revoke: Option<String>,
}

/// A PKCE pair: the secret kept here, and the digest sent to the provider.
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    pub fn new() -> Self {
        let verifier = random_value();
        Self {
            challenge: challenge_for(&verifier),
            verifier,
        }
    }
}

/// 256 bits of unguessable text, for a PKCE verifier, a `state` or a `nonce`.
///
/// Two v4 uuids rather than a random-bytes dependency: each is 122 bits from the
/// system's cryptographic generator, so the pair is well past what any of these three
/// uses needs, and the crate is already here for document ids.
pub fn random_value() -> String {
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(Uuid::new_v4().as_bytes());
    bytes[16..].copy_from_slice(Uuid::new_v4().as_bytes());
    URL_SAFE_NO_PAD.encode(bytes)
}

/// The `S256` challenge: base64url of the SHA-256 of the verifier, unpadded.
pub fn challenge_for(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// Check a provider configuration document and pull the endpoints out of it.
///
/// The issuer has to be exactly who we expected, and every endpoint has to be on that
/// same origin. Without the origin check, a document that passed the issuer check could
/// still send the authorization code to another host.
pub fn read_discovery(value: &Value) -> ChatResult<Discovery> {
    let bad = || {
        ChatError::retryable(
            ChatErrorCode::Discovery,
            "ChatGPT sign-in configuration could not be verified.",
        )
    };
    if value.get("issuer").and_then(Value::as_str) != Some(ISSUER) {
        return Err(bad());
    }
    let expected = Url::parse(ISSUER).map_err(|_| bad())?.origin();
    let endpoint = |key: &str| -> ChatResult<String> {
        let text = value.get(key).and_then(Value::as_str).ok_or_else(bad)?;
        let url = Url::parse(text).map_err(|_| bad())?;
        if url.origin() != expected {
            return Err(bad());
        }
        Ok(text.to_string())
    };
    let revoke = match value.get("revocation_endpoint") {
        None | Some(Value::Null) => None,
        Some(_) => Some(endpoint("revocation_endpoint")?),
    };
    Ok(Discovery {
        authorize: endpoint("authorization_endpoint")?,
        token: endpoint("token_endpoint")?,
        jwks: endpoint("jwks_uri")?,
        revoke,
    })
}

/// What the authorize request has to say.
pub struct AuthorizeRequest<'a> {
    pub discovery: &'a Discovery,
    pub redirect_uri: &'a str,
    pub state: &'a str,
    pub nonce: &'a str,
    pub challenge: &'a str,
    pub host_id: &'a str,
    /// The id from a previous registration, when there is one. Its absence is what makes
    /// this a registration rather than a sign-in.
    pub client_id: Option<&'a str>,
    /// The email from the last successful sign-in, so the provider can preselect the
    /// account. Only the email: a saved ID token would end up in the browser's process
    /// arguments, where it is readable by anything on the machine.
    pub login_hint: Option<&'a str>,
}

pub fn authorize_url(request: &AuthorizeRequest<'_>) -> ChatResult<String> {
    let mut url = Url::parse(&request.discovery.authorize).map_err(|_| {
        ChatError::new(
            ChatErrorCode::Discovery,
            "ChatGPT gave an unusable sign-in address.",
        )
    })?;
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("client_id", request.client_id.unwrap_or(FIRST_SIGN_IN))
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", request.redirect_uri)
            .append_pair("scope", &super::SCOPES.join(" "))
            .append_pair("resource", RESOURCE)
            .append_pair("state", request.state)
            .append_pair("nonce", request.nonce)
            .append_pair("code_challenge_method", "S256")
            .append_pair("code_challenge", request.challenge)
            .append_pair("ext_agent_host_id", request.host_id);
        // The name is what registers the app, so it is sent only when registering. Sent
        // again alongside an issued id, it is at best ignored and at worst a second app.
        if request.client_id.is_none() {
            query.append_pair("agent_name_hint", AGENT_NAME);
        }
        if let Some(email) = request.login_hint {
            query.append_pair("login_hint", email);
        }
    }
    Ok(url.to_string())
}

/// What a token response granted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Granted {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: i64,
    pub scopes: Vec<String>,
    /// Present on an exchange, and sometimes on a refresh. Never trusted until verified.
    pub id_token: Option<String>,
}

/// Read a token response, refusing anything incomplete.
///
/// `granted_before` is for refresh, where the provider may leave `scope` out to mean
/// "the same as before". Reading that as "nothing was granted" would silently downgrade
/// a working sign-in, so the previous grant is the fallback rather than an empty list.
pub fn read_token_response(
    value: &Value,
    granted_before: Option<&[String]>,
) -> ChatResult<Granted> {
    let bad = |what: &str| {
        ChatError::new(
            ChatErrorCode::Provider,
            format!("ChatGPT returned {what}. Please sign in again."),
        )
    };

    let scopes: Vec<String> = match value.get("scope").and_then(Value::as_str) {
        Some(text) => text.split_whitespace().map(str::to_string).collect(),
        None => granted_before
            .ok_or_else(|| bad("no list of granted permissions"))?
            .to_vec(),
    };

    let access_token = value
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| bad("no access token"))?;

    // Checked rather than assumed: the token is about to be put in an Authorization
    // header, and the scheme to use is whatever this says.
    let bearer = value
        .get("token_type")
        .and_then(Value::as_str)
        .is_some_and(|t| t.eq_ignore_ascii_case("bearer"));
    if !bearer {
        return Err(bad("a token of an unexpected kind"));
    }

    let expires_in = value
        .get("expires_in")
        .and_then(Value::as_i64)
        .filter(|s| *s > 0)
        .ok_or_else(|| bad("a token with no usable lifetime"))?;

    let refresh_token = value
        .get("refresh_token")
        .and_then(Value::as_str)
        .filter(|t| !t.is_empty())
        .map(str::to_string);
    // Only required when the permission that produces one was granted. Without it,
    // sign-in still works; it just will not survive the access token's lifetime.
    if refresh_token.is_none() && scopes.iter().any(|s| s == "offline_access") {
        return Err(bad("no way to stay signed in"));
    }

    let id_token = match value.get("id_token") {
        None | Some(Value::Null) => None,
        Some(Value::String(token)) if !token.is_empty() => Some(token.clone()),
        Some(_) => return Err(bad("an unusable identity")),
    };

    Ok(Granted {
        access_token: access_token.to_string(),
        refresh_token,
        expires_in,
        scopes,
        id_token,
    })
}

/// Who the provider says this is, once the ID token has been verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub subject: String,
    pub email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
    email: Option<String>,
    nonce: Option<String>,
    /// The party the token was issued to, when it differs from the audience.
    azp: Option<String>,
}

// ---------------------------------------------------------------------------
// The network
// ---------------------------------------------------------------------------

/// The one HTTP client everything here uses.
///
/// Shared so that its two settings are impossible to forget: no redirects, because a
/// redirect resends the body and the body is a credential or a document; and a timeout,
/// because a request with none is a window that never closes.
pub fn http() -> ChatResult<&'static reqwest::Client> {
    static CLIENT: OnceLock<Option<reqwest::Client>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(TIMEOUT)
                // A redirect on a token request would resend the body, and the body is
                // the authorization code or the refresh token. There is nowhere else
                // these may go, and the endpoint has already been verified.
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .ok()
        })
        .as_ref()
        .ok_or_else(|| {
            ChatError::new(
                ChatErrorCode::Network,
                "Studio could not start a secure connection.",
            )
        })
}

fn unreachable(error: &reqwest::Error) -> ChatError {
    let message = if error.is_timeout() {
        "ChatGPT did not respond in time.".to_string()
    } else {
        "ChatGPT could not be reached. Check your connection.".to_string()
    };
    ChatError::retryable(ChatErrorCode::Network, message)
}

pub async fn discover() -> ChatResult<Discovery> {
    let url = format!("{ISSUER}/.well-known/openid-configuration");
    let response = http()?.get(url).send().await.map_err(|e| unreachable(&e))?;
    let value: Value = response.json().await.map_err(|_| {
        ChatError::retryable(
            ChatErrorCode::Discovery,
            "ChatGPT sign-in configuration could not be read.",
        )
    })?;
    read_discovery(&value)
}

/// The `application/x-www-form-urlencoded` body these endpoints take.
fn form_body(fields: &[(&str, &str)]) -> String {
    let mut body = url::form_urlencoded::Serializer::new(String::new());
    for (name, value) in fields {
        body.append_pair(name, value);
    }
    body.finish()
}

/// POST a form to the token endpoint and read what comes back.
async fn post_form(url: &str, form: &[(&str, &str)]) -> ChatResult<Value> {
    let response = http()?
        .post(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(form_body(form))
        .send()
        .await
        .map_err(|e| unreachable(&e))?;
    let status = response.status();
    let value: Value = response.json().await.unwrap_or(Value::Null);
    if status.is_success() {
        return Ok(value);
    }
    Err(provider_error(&value, status))
}

/// Turn a refusal into something worth showing.
///
/// `invalid_grant` is singled out because it is the one that is not a fault: it means
/// the stored grant has been revoked or has aged out, and the answer is to sign in
/// again rather than to retry.
fn provider_error(value: &Value, status: reqwest::StatusCode) -> ChatError {
    let code = value.get("error").and_then(Value::as_str).unwrap_or("");
    let described = value
        .get("error_description")
        .and_then(Value::as_str)
        .filter(|d| !d.is_empty());
    match code {
        "invalid_grant" => ChatError::new(
            ChatErrorCode::Expired,
            "Your ChatGPT sign-in is no longer valid. Please sign in again.",
        ),
        "access_denied" => ChatError::cancelled(),
        _ => {
            let message = described
                .map(str::to_string)
                .unwrap_or_else(|| format!("ChatGPT refused the request ({status})."));
            ChatError {
                code: ChatErrorCode::Provider,
                message,
                retryable: status.is_server_error(),
            }
        }
    }
}

/// Trade the one-time code for tokens.
pub async fn exchange(
    discovery: &Discovery,
    client_id: &str,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> ChatResult<Granted> {
    let value = post_form(
        &discovery.token,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("code", code),
            ("code_verifier", verifier),
            // Has to match the authorize request exactly, not merely be equivalent.
            ("redirect_uri", redirect_uri),
            ("resource", RESOURCE),
        ],
    )
    .await?;
    read_token_response(&value, None)
}

/// Trade the refresh token for a new access token, and usually a new refresh token.
pub async fn refresh(
    discovery: &Discovery,
    client_id: &str,
    refresh_token: &str,
    granted_before: &[String],
) -> ChatResult<Granted> {
    let value = post_form(
        &discovery.token,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("refresh_token", refresh_token),
            ("resource", RESOURCE),
        ],
    )
    .await?;
    read_token_response(&value, Some(granted_before))
}

/// Tell the provider the grant is finished with.
///
/// The refresh token is what is revoked, because that is what the renewable session
/// hangs off. Best effort by design: the local copy is deleted whatever happens here,
/// since a person who asked to sign out has signed out.
pub async fn revoke(discovery: &Discovery, client_id: &str, refresh_token: &str) -> ChatResult<()> {
    let Some(url) = discovery.revoke.as_deref() else {
        return Ok(());
    };
    let response = http()?
        .post(url)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(form_body(&[
            ("token", refresh_token),
            ("token_type_hint", "refresh_token"),
            ("client_id", client_id),
        ]))
        .send()
        .await
        .map_err(|e| unreachable(&e))?;
    if response.status().is_success() {
        return Ok(());
    }
    Err(ChatError::retryable(
        ChatErrorCode::Provider,
        "Studio removed its copy of your credentials, but ChatGPT did not confirm the \
         disconnection. You can remove Studio in ChatGPT settings.",
    ))
}

/// Verify an ID token and return who it says this is.
///
/// Fetched fresh each time rather than cached. Sign-in and refresh are rare, and a cache
/// would need invalidating when the provider rotates keys, which is a way to turn a key
/// rotation into a sign-out.
pub async fn verify_identity(
    discovery: &Discovery,
    id_token: &str,
    client_id: &str,
    nonce: Option<&str>,
) -> ChatResult<Identity> {
    let response = http()?
        .get(&discovery.jwks)
        .send()
        .await
        .map_err(|_| ChatError::identity_unavailable())?;
    if !response.status().is_success() {
        return Err(ChatError::identity_unavailable());
    }
    let keys: JwkSet = response
        .json()
        .await
        .map_err(|_| ChatError::identity_unavailable())?;
    verify_with(&keys, id_token, client_id, nonce)
}

/// The verification itself, over keys already in hand.
///
/// Separated from fetching them so it can be tested, and so that a key service that
/// cannot be reached is never mistaken for a token that does not verify. The first is
/// an outage; the second means somebody is lying about who they are.
pub fn verify_with(
    keys: &JwkSet,
    id_token: &str,
    client_id: &str,
    nonce: Option<&str>,
) -> ChatResult<Identity> {
    let rejected = || {
        ChatError::new(
            ChatErrorCode::Identity,
            "The ChatGPT identity could not be verified. Please sign in again.",
        )
    };

    let header = decode_header(id_token).map_err(|_| rejected())?;
    let key = header
        .kid
        .as_deref()
        .and_then(|kid| keys.find(kid))
        .ok_or_else(|| {
            // A key id we have never seen is more often a rotation we have not caught up
            // with than a forgery, and signing somebody out over it would be wrong.
            ChatError::identity_unavailable()
        })?;
    let decoding = DecodingKey::from_jwk(key).map_err(|_| ChatError::identity_unavailable())?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[ISSUER]);
    validation.set_audience(&[client_id]);
    validation.set_required_spec_claims(&["iss", "aud", "exp", "iat", "sub"]);
    validation.leeway = 5;

    let token = decode::<Claims>(id_token, &decoding, &validation).map_err(|_| rejected())?;
    let claims = token.claims;

    if claims.sub.is_empty() {
        return Err(rejected());
    }
    // Only on a fresh sign-in is there a nonce to check. On a refresh there is no
    // attempt to bind the token to, and demanding one would reject every refresh.
    if let Some(expected) = nonce {
        if claims.nonce.as_deref() != Some(expected) {
            return Err(rejected());
        }
    }
    // When present, the authorized party has to be us. Otherwise a token minted for
    // another client that happens to list us in its audience would pass.
    if let Some(azp) = claims.azp.as_deref() {
        if azp != client_id {
            return Err(rejected());
        }
    }

    Ok(Identity {
        subject: claims.sub,
        email: claims.email.filter(|e| !e.is_empty()),
    })
}

/// The base64 (not base64url) SHA-256 of a script, for a `script-src` hash in a Content
/// Security Policy. Here because the callback page carries one.
pub fn csp_hash(script: &str) -> String {
    STANDARD.encode(Sha256::digest(script.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn discovery_doc() -> Value {
        json!({
            "issuer": ISSUER,
            "authorization_endpoint": "https://auth.openai.com/api/accounts/authorize",
            "token_endpoint": "https://auth.openai.com/api/accounts/oauth/token",
            "jwks_uri": "https://auth.openai.com/.well-known/jwks.json",
            "revocation_endpoint": "https://auth.openai.com/api/accounts/oauth/revoke",
        })
    }

    #[test]
    fn the_challenge_is_the_unpadded_digest_of_the_verifier() {
        // The one worked example in RFC 7636 appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            challenge_for(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert!(
            !challenge_for(verifier).contains('='),
            "padding would be sent"
        );
    }

    #[test]
    fn two_attempts_never_share_a_verifier() {
        let (a, b) = (Pkce::new(), Pkce::new());
        assert_ne!(a.verifier, b.verifier);
        assert_ne!(a.challenge, b.challenge);
        assert_eq!(a.challenge, challenge_for(&a.verifier));
        // 32 bytes, base64url, unpadded.
        assert_eq!(a.verifier.len(), 43);
    }

    #[test]
    fn a_good_configuration_is_accepted() {
        let found = read_discovery(&discovery_doc()).unwrap();
        assert_eq!(
            found.token,
            "https://auth.openai.com/api/accounts/oauth/token"
        );
        assert!(found.revoke.is_some());
    }

    #[test]
    fn an_endpoint_on_another_host_is_refused() {
        // The issuer is right, so an issuer check alone would let this through, and the
        // authorization code would be posted to somebody else.
        let mut doc = discovery_doc();
        doc["token_endpoint"] = json!("https://auth.openai.com.evil.test/token");
        let error = read_discovery(&doc).unwrap_err();
        assert_eq!(error.code, ChatErrorCode::Discovery);

        let mut doc = discovery_doc();
        doc["jwks_uri"] = json!("http://auth.openai.com/.well-known/jwks.json");
        assert!(read_discovery(&doc).is_err(), "a downgrade to http");
    }

    #[test]
    fn the_wrong_issuer_is_refused() {
        let mut doc = discovery_doc();
        doc["issuer"] = json!("https://auth.openai.com/");
        assert!(read_discovery(&doc).is_err(), "not an exact match");
    }

    #[test]
    fn a_provider_without_revocation_is_still_usable() {
        let mut doc = discovery_doc();
        doc.as_object_mut().unwrap().remove("revocation_endpoint");
        assert_eq!(read_discovery(&doc).unwrap().revoke, None);
    }

    fn request<'a>(discovery: &'a Discovery, client_id: Option<&'a str>) -> AuthorizeRequest<'a> {
        AuthorizeRequest {
            discovery,
            redirect_uri: "http://127.0.0.1:51234/auth/callback",
            state: "state-value",
            nonce: "nonce-value",
            challenge: "challenge-value",
            host_id: "urn:uuid:1234",
            client_id,
            login_hint: None,
        }
    }

    fn query(url: &str) -> std::collections::HashMap<String, String> {
        Url::parse(url)
            .unwrap()
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect()
    }

    #[test]
    fn a_first_sign_in_registers_the_app() {
        let discovery = read_discovery(&discovery_doc()).unwrap();
        let url = authorize_url(&request(&discovery, None)).unwrap();
        let q = query(&url);
        assert_eq!(q["client_id"], FIRST_SIGN_IN);
        assert_eq!(q["agent_name_hint"], AGENT_NAME);
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["resource"], RESOURCE);
        assert_eq!(q["ext_agent_host_id"], "urn:uuid:1234");
        assert_eq!(q["scope"], super::super::SCOPES.join(" "));
        assert!(q["scope"].contains(super::super::SCOPE_INFERENCE));
    }

    #[test]
    fn a_later_sign_in_reuses_the_registration_instead_of_making_another() {
        let discovery = read_discovery(&discovery_doc()).unwrap();
        let url = authorize_url(&request(&discovery, Some("oaiapp_abc"))).unwrap();
        let q = query(&url);
        assert_eq!(q["client_id"], "oaiapp_abc");
        assert!(
            !q.contains_key("agent_name_hint"),
            "sending the name again would register a second app"
        );
        assert_eq!(q["ext_agent_host_id"], "urn:uuid:1234");
    }

    #[test]
    fn only_the_email_is_offered_as_a_hint() {
        let discovery = read_discovery(&discovery_doc()).unwrap();
        let mut req = request(&discovery, Some("oaiapp_abc"));
        req.login_hint = Some("a@example.com");
        let q = query(&authorize_url(&req).unwrap());
        assert_eq!(q["login_hint"], "a@example.com");
        // A saved ID token in a browser URL would be readable from the process list.
        assert!(!q.contains_key("id_token_hint"));
    }

    fn token_response() -> Value {
        json!({
            "access_token": "at",
            "refresh_token": "rt",
            "token_type": "Bearer",
            "expires_in": 3600,
            "scope": "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct",
            "id_token": "idt",
        })
    }

    #[test]
    fn a_complete_grant_is_read() {
        let granted = read_token_response(&token_response(), None).unwrap();
        assert_eq!(granted.access_token, "at");
        assert_eq!(granted.refresh_token.as_deref(), Some("rt"));
        assert_eq!(granted.expires_in, 3600);
        assert_eq!(granted.id_token.as_deref(), Some("idt"));
        assert!(granted
            .scopes
            .iter()
            .any(|s| s == super::super::SCOPE_INFERENCE));
    }

    #[test]
    fn a_grant_that_cannot_be_kept_is_refused() {
        // offline_access was granted, so there must be a refresh token to go with it.
        let mut value = token_response();
        value.as_object_mut().unwrap().remove("refresh_token");
        assert!(read_token_response(&value, None).is_err());
    }

    #[test]
    fn a_grant_without_offline_access_needs_no_refresh_token() {
        let mut value = token_response();
        value.as_object_mut().unwrap().remove("refresh_token");
        value["scope"] = json!("openid profile email");
        let granted = read_token_response(&value, None).unwrap();
        assert_eq!(granted.refresh_token, None);
    }

    #[test]
    fn an_unusable_token_is_refused_rather_than_sent() {
        for (field, value) in [
            ("token_type", json!("mac")),
            ("expires_in", json!(0)),
            ("expires_in", json!("3600")),
            ("access_token", json!("")),
        ] {
            let mut response = token_response();
            response[field] = value.clone();
            assert!(
                read_token_response(&response, None).is_err(),
                "{field} = {value} was accepted"
            );
        }
    }

    #[test]
    fn a_refresh_that_says_nothing_about_scope_keeps_what_was_granted() {
        let mut value = token_response();
        value.as_object_mut().unwrap().remove("scope");
        let before = vec![
            "offline_access".to_string(),
            super::super::SCOPE_INFERENCE.to_string(),
        ];

        // Without the fallback this is unreadable, which is the honest answer when
        // nothing is known about what was granted.
        assert!(read_token_response(&value, None).is_err());

        let granted = read_token_response(&value, Some(&before)).unwrap();
        assert_eq!(granted.scopes, before, "the grant must not quietly shrink");
    }

    #[test]
    fn a_refusal_that_means_sign_in_again_is_not_reported_as_a_fault() {
        let error = provider_error(
            &json!({ "error": "invalid_grant" }),
            reqwest::StatusCode::BAD_REQUEST,
        );
        assert_eq!(error.code, ChatErrorCode::Expired);
        assert!(!error.retryable, "retrying a dead grant cannot help");

        let error = provider_error(
            &json!({ "error": "access_denied" }),
            reqwest::StatusCode::FORBIDDEN,
        );
        assert_eq!(error.code, ChatErrorCode::Cancelled);
    }

    #[test]
    fn the_provider_having_a_bad_day_is_worth_retrying() {
        let error = provider_error(&Value::Null, reqwest::StatusCode::BAD_GATEWAY);
        assert!(error.retryable);
        let error = provider_error(&Value::Null, reqwest::StatusCode::BAD_REQUEST);
        assert!(!error.retryable);
    }

    #[test]
    fn an_identity_signed_by_an_unknown_key_is_not_called_invalid() {
        // An empty key set is what a rotation or a partial outage looks like. Calling
        // that an invalid identity would sign a person out over somebody else's problem.
        let keys = JwkSet { keys: Vec::new() };
        let token = "eyJhbGciOiJSUzI1NiIsImtpZCI6ImsxIn0.eyJzdWIiOiJ1In0.c2ln";
        let error = verify_with(&keys, token, "oaiapp_abc", None).unwrap_err();
        assert_eq!(error.code, ChatErrorCode::Network);
        assert!(error.retryable);
    }

    #[test]
    fn something_that_is_not_a_token_is_refused() {
        let keys = JwkSet { keys: Vec::new() };
        let error = verify_with(&keys, "not-a-jwt", "oaiapp_abc", None).unwrap_err();
        assert_eq!(error.code, ChatErrorCode::Identity);
    }

    #[test]
    fn the_policy_hash_is_standard_base64() {
        // The CSP grammar takes base64, not base64url, so a digest with - or _ in it
        // would silently fail to match and the page's script would not run.
        let hash = csp_hash("history.replaceState(null, \"\", \"/auth/complete\");");
        assert!(hash.ends_with('='), "padding is part of the grammar");
        assert!(!hash.contains('-') && !hash.contains('_'));
    }
}
