//! Talking to a model, and what that needs from this machine.
//!
//! The catalogue in `studio-agent` is document capability: it knows what can be done and
//! says nothing about who is asking or how they got here. Reaching a model is host
//! capability, the same judgement that put `probe` behind the `Host` trait, so it lives
//! in the shell beside the other things only a desktop process can do: a loopback
//! listener, the system browser, and a file on disk that only its owner can read.
//!
//! The one rule that shapes everything here: the webview never sees a token. It sends a
//! message and renders events. A bug in the panel cannot leak a credential it was never
//! given, and the panel is the part most likely to have bugs.

mod callback;
mod client;
pub mod credentials;
mod error;
mod oauth;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

use crate::state::AppState;
use credentials::{ChatStatus, Credentials};
pub use error::{ChatError, ChatErrorCode, ChatResult};

/// Sign-in is through a person's own ChatGPT plan rather than an API key, so the scopes
/// are the two halves of that: who you are, and permission to spend your plan.
///
/// `offline_access` is what makes a refresh token possible, which is what makes signing
/// in once rather than every launch possible.
pub const SCOPES: &[&str] = &[
    "openid",
    "profile",
    "email",
    "offline_access",
    "resource.invoke",
    SCOPE_INFERENCE,
];

/// The scope that actually permits inference against the plan.
///
/// Named on its own because it is the one that can be withheld while sign-in still
/// succeeds, and that case needs saying differently from being signed out.
pub const SCOPE_INFERENCE: &str = "chatgpt.tokens.use.direct";

/// How long the browser half of sign-in may take before the listener gives up. Long
/// enough to find a password and a second factor, short enough that a forgotten tab does
/// not hold a port open all day.
const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(300);

/// Who is signed in, as far as the stored credentials say.
///
/// Deliberately does not touch the network: it is called when the panel mounts, and a
/// panel that cannot say anything until a request completes is a panel that flickers.
#[tauri::command]
pub fn chat_status(app: AppHandle) -> ChatStatus {
    credentials::load(&app)
        .as_ref()
        .map(ChatStatus::from)
        .unwrap_or_else(ChatStatus::signed_out)
}

/// Sign in, or make an existing sign-in usable again.
///
/// Two paths, and the cheap one comes first. Credentials that have merely aged out are
/// refreshed in place, because sending somebody to a browser to prove again what they
/// proved last week is a worse experience for no more safety. The browser is for when
/// there is nothing left to refresh.
#[tauri::command]
pub async fn chat_sign_in(app: AppHandle) -> ChatResult<ChatStatus> {
    let _guard = SignIn::begin()?;

    if let Some(stored) = credentials::load(&app) {
        if !stored.needs_refresh(credentials::now_unix()) {
            return Ok(ChatStatus::from(&stored));
        }
        match renew(&app, &stored).await {
            Ok(fresh) => return Ok(ChatStatus::from(&fresh)),
            // A grant that is genuinely finished is the one case worth a browser. Any
            // other failure is reported rather than papered over with a sign-in prompt,
            // which would hide an outage behind a confusing request to authenticate.
            Err(error) if error.code == ChatErrorCode::Expired => {}
            Err(error) => return Err(error),
        }
    }

    authorize(&app).await
}

/// Sign out: tell the provider, then forget the tokens.
///
/// The local copy goes whatever the provider says. Somebody who asked to sign out has
/// signed out, and leaving tokens on disk because a request failed would be the wrong
/// way round. A provider that did not confirm is reported, so they can finish the job in
/// ChatGPT settings if they want to.
#[tauri::command]
pub async fn chat_sign_out(app: AppHandle) -> ChatStatus {
    if let (Some(stored), Some(client_id)) =
        (credentials::load(&app), credentials::registration(&app))
    {
        if let Some(token) = stored.refresh_token.as_deref() {
            if let Ok(discovery) = oauth::discover().await {
                let _ = oauth::revoke(&discovery, &client_id, token).await;
            }
        }
    }
    credentials::clear(&app);
    ChatStatus::signed_out()
}

/// Exchange the refresh token for a fresh access token.
async fn renew(app: &AppHandle, stored: &Credentials) -> ChatResult<Credentials> {
    let token = stored
        .refresh_token
        .as_deref()
        .ok_or_else(|| ChatError::new(ChatErrorCode::Expired, "This sign-in cannot be renewed."))?;
    let discovery = oauth::discover().await?;
    let granted = oauth::refresh(&discovery, &stored.client_id, token, &stored.scopes).await?;

    // A refresh does not always carry an identity. When it does, it has to still be the
    // same account: a refresh that came back as somebody else is not a refresh.
    let mut identity = None;
    if let Some(id_token) = granted.id_token.as_deref() {
        let found = oauth::verify_identity(&discovery, id_token, &stored.client_id, None).await?;
        if found.subject != stored.subject {
            return Err(ChatError::new(
                ChatErrorCode::Identity,
                "ChatGPT returned a different account. Please sign in again.",
            ));
        }
        identity = Some(found);
    }

    let fresh = Credentials {
        client_id: stored.client_id.clone(),
        subject: stored.subject.clone(),
        email: identity
            .and_then(|i| i.email)
            .or_else(|| stored.email.clone()),
        access_token: granted.access_token,
        // A rotated refresh token replaces the old one. The old one is spent, so keeping
        // it would store something that no longer works.
        refresh_token: granted
            .refresh_token
            .or_else(|| stored.refresh_token.clone()),
        expires_at_unix: credentials::now_unix() + granted.expires_in,
        scopes: granted.scopes,
    };
    credentials::save(app, &fresh)?;
    Ok(fresh)
}

/// The browser half: register if needed, authorize, exchange, verify, store.
async fn authorize(app: &AppHandle) -> ChatResult<ChatStatus> {
    let discovery = oauth::discover().await?;
    let host_id = credentials::host_id(app)?;
    let known_client = credentials::registration(app);
    let previous_email = credentials::load(app).and_then(|c| c.email);

    // Bound before the browser opens, so the redirect URI in the authorize request is
    // one that something is already listening on.
    let listener = callback::Listener::bind()?;
    let redirect_uri = listener.redirect_uri();
    let pkce = oauth::Pkce::new();
    let state = oauth::random_value();
    let nonce = oauth::random_value();

    let url = oauth::authorize_url(&oauth::AuthorizeRequest {
        discovery: &discovery,
        redirect_uri: &redirect_uri,
        state: &state,
        nonce: &nonce,
        challenge: &pkce.challenge,
        host_id: &host_id,
        client_id: known_client.as_deref(),
        login_hint: previous_email.as_deref(),
    })?;
    open_browser(&url)?;

    let waiting = {
        let (state, saved) = (state.clone(), known_client.clone());
        tauri::async_runtime::spawn_blocking(move || {
            listener.wait(&state, saved.as_deref(), SIGN_IN_TIMEOUT)
        })
    };
    let returned = waiting.await.map_err(|_| {
        ChatError::new(
            ChatErrorCode::Cancelled,
            "Sign-in stopped before it finished.",
        )
    })??;

    // Before the exchange: the registration is real now, and the code may still fail.
    credentials::save_registration(app, &returned.client_id)?;

    let granted = oauth::exchange(
        &discovery,
        &returned.client_id,
        &returned.code,
        &pkce.verifier,
        &redirect_uri,
    )
    .await?;

    let id_token = granted.id_token.as_deref().ok_or_else(|| {
        ChatError::new(
            ChatErrorCode::Identity,
            "ChatGPT did not say which account this is. Please try signing in again.",
        )
    })?;
    let identity =
        oauth::verify_identity(&discovery, id_token, &returned.client_id, Some(&nonce)).await?;

    let fresh = Credentials {
        client_id: returned.client_id,
        subject: identity.subject,
        email: identity.email,
        access_token: granted.access_token,
        refresh_token: granted.refresh_token,
        expires_at_unix: credentials::now_unix() + granted.expires_in,
        scopes: granted.scopes,
    };
    credentials::save(app, &fresh)?;
    Ok(ChatStatus::from(&fresh))
}

/// Hand a URL to whatever the person uses to browse.
///
/// An argument vector, never a command line: the URL comes from the provider's own
/// configuration, and a URL assembled into a shell string is a URL that can carry
/// something other than a URL. The same rule the FFmpeg calls follow.
fn open_browser(url: &str) -> ChatResult<()> {
    #[cfg(target_os = "macos")]
    let (program, args) = ("open", vec![url]);
    #[cfg(target_os = "windows")]
    let (program, args) = ("rundll32", vec!["url.dll,FileProtocolHandler", url]);
    #[cfg(all(unix, not(target_os = "macos")))]
    let (program, args) = ("xdg-open", vec![url]);

    let opened = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    match opened {
        Ok(status) if status.success() => Ok(()),
        _ => Err(ChatError::new(
            ChatErrorCode::Browser,
            "Studio could not open your browser. Check your default browser and try again.",
        )),
    }
}

/// One sign-in at a time.
///
/// Two at once would each bind their own port and open their own tab, and whichever
/// finished last would overwrite the other. The guard releases on every exit path,
/// including the error ones, because it is released by being dropped.
struct SignIn;

static IN_PROGRESS: AtomicBool = AtomicBool::new(false);

impl SignIn {
    fn begin() -> ChatResult<Self> {
        if IN_PROGRESS.swap(true, Ordering::SeqCst) {
            return Err(ChatError::new(
                ChatErrorCode::Busy,
                "A sign-in is already in progress. Finish it in your browser, or wait for \
                 it to time out.",
            ));
        }
        Ok(Self)
    }
}

impl Drop for SignIn {
    fn drop(&mut self) {
        IN_PROGRESS.store(false, Ordering::SeqCst);
    }
}

/// Emitted as `chat://delta` while an answer is being written.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatDelta {
    pub text: String,
}

/// Standing guidance, sent with every turn rather than put in the conversation.
///
/// Not a message, because it is not something anybody said and should not be answered
/// as though it were. Resent each turn because the API keeps nothing between them, which
/// is the same reason the conversation itself is carried in full.
const INSTRUCTIONS: &str = "\
You are the assistant inside Studio, a desktop video editor. The person you are talking \
to has a project open and can see its timeline, media and viewer.

Be brief and concrete. Answer in plain sentences, not lists, unless a list is genuinely \
the shape of the answer. Use timecodes the way an editor does.

You are shown a summary of the open project below. It is a summary: it says how many \
clips and tracks a sequence has, not what is on them. When you need detail you do not \
have, say what you would need rather than guessing, and never state a specific clip, \
time or name you were not told.

You cannot change the project yet. If you are asked to make an edit, say plainly that \
making edits is not switched on yet, and say what you would have done.";

/// The guidance plus what is open right now.
///
/// Rebuilt every turn rather than once per conversation, so an answer is never given
/// against a project that has since been closed, saved as something else, or edited by
/// hand while the conversation was sitting idle.
fn instructions(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    let mut slot = state.session();
    let Some(session) = slot.as_mut() else {
        return format!("{INSTRUCTIONS}\n\nNo project is open.");
    };
    match studio_agent::queries::overview(&session.editor) {
        Ok(overview) => format!(
            "{INSTRUCTIONS}\n\nThe open project:\n{}",
            serde_json::to_string_pretty(&overview).unwrap_or_default()
        ),
        Err(_) => format!("{INSTRUCTIONS}\n\nNo project is open."),
    }
}

/// The conversation, for as long as the app is running.
///
/// Items rather than messages, because a turn holds more than what was said: a reasoning
/// model produces items it expects to see again, and a tool call and its result are part
/// of the record too. Deliberately not written to disk: nothing here asked to be kept.
static CONVERSATION: Mutex<Vec<Value>> = Mutex::new(Vec::new());

/// The model to use, chosen once per run.
///
/// Not written down as a slug, because which models a plan offers changes and a slug in
/// the source goes stale silently. The service lists them in its own order, and the
/// first is its own answer to which one to use.
static MODEL: Mutex<Option<String>> = Mutex::new(None);

static ANSWERING: AtomicBool = AtomicBool::new(false);

/// Say something, and get the answer.
///
/// The answer also arrives piece by piece as `chat://delta` while this is still running,
/// so the panel can show it being written. What this returns is the same text, whole:
/// the panel replaces what it has been showing, which is also how it recovers if it
/// missed a piece.
#[tauri::command]
pub async fn chat_send(app: AppHandle, message: String) -> ChatResult<String> {
    let message = message.trim().to_string();
    if message.is_empty() {
        return Ok(String::new());
    }
    let _guard = Answering::begin()?;

    let token = access_token(&app).await?;
    let model = model(&token).await?;

    // The question joins the conversation before the answer is asked for, so the model
    // sees it in order. If the ask fails it comes back out: a question nobody answered
    // would otherwise be asked again, silently, with the next one.
    let asked = {
        let mut conversation = CONVERSATION.lock().unwrap_or_else(|e| e.into_inner());
        conversation.push(json!({ "role": "user", "content": message }));
        conversation.clone()
    };

    let instructions = instructions(&app);
    let answered = client::ask(
        &token,
        &client::Turn {
            model: &model,
            input: &asked,
            instructions: &instructions,
        },
        |text| {
            let _ = app.emit(
                EVENT_DELTA,
                ChatDelta {
                    text: text.to_string(),
                },
            );
        },
    )
    .await;

    let mut conversation = CONVERSATION.lock().unwrap_or_else(|e| e.into_inner());
    match answered {
        Ok(response) => {
            conversation.extend(client::output_items(&response));
            Ok(client::output_text(&response))
        }
        Err(error) => {
            conversation.pop();
            Err(error)
        }
    }
}

/// Start again, forgetting what was said.
#[tauri::command]
pub fn chat_reset() {
    CONVERSATION
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
}

pub const EVENT_DELTA: &str = "chat://delta";

/// A token that will still be good when the request arrives.
///
/// Refreshed here rather than on a timer: a timer runs while nobody is asking anything,
/// and the only moment a token's freshness matters is the moment before it is used.
async fn access_token(app: &AppHandle) -> ChatResult<String> {
    let stored = credentials::load(app).ok_or_else(ChatError::signed_out)?;
    let stored = if stored.needs_refresh(credentials::now_unix()) {
        renew(app, &stored).await?
    } else {
        stored
    };
    if !stored.may_infer() {
        return Err(ChatError::new(
            ChatErrorCode::ScopeDenied,
            "Studio was not given permission to use your ChatGPT plan. Sign out and sign \
             in again, allowing it.",
        ));
    }
    Ok(stored.access_token)
}

/// Which model to ask, fetched once and kept.
async fn model(token: &str) -> ChatResult<String> {
    if let Some(chosen) = MODEL.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return Ok(chosen);
    }
    let offered = client::list_models(token).await?;
    let chosen = offered
        .into_iter()
        .next()
        .map(|model| model.slug)
        .ok_or_else(|| {
            ChatError::new(
                ChatErrorCode::Provider,
                "Your ChatGPT plan does not offer a model Studio can use.",
            )
        })?;
    *MODEL.lock().unwrap_or_else(|e| e.into_inner()) = Some(chosen.clone());
    Ok(chosen)
}

/// One question at a time.
///
/// Two answers streaming into one panel would interleave, and the conversation they both
/// appended to would end up in an order neither of them asked for.
struct Answering;

impl Answering {
    fn begin() -> ChatResult<Self> {
        if ANSWERING.swap(true, Ordering::SeqCst) {
            return Err(ChatError::new(
                ChatErrorCode::Busy,
                "Studio is still answering the last question.",
            ));
        }
        Ok(Self)
    }
}

impl Drop for Answering {
    fn drop(&mut self) {
        ANSWERING.store(false, Ordering::SeqCst);
    }
}
