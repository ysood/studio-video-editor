//! What is kept between launches so you do not sign in every time.
//!
//! Two things live here, and they are different in kind. The host id identifies this
//! installation to OpenAI and is not a secret: it must be the same value on every
//! sign-in from this machine, which is the whole point of it. The tokens are secrets,
//! and the difference shows in how each is treated when something goes wrong.
//!
//! A recents list that cannot be read is an inconvenience, so `recents.rs` shrugs and
//! starts over. Credentials are not like that. A file that will not parse, or that was
//! written by a later version, is left exactly as it is rather than overwritten, and the
//! app behaves as though nobody is signed in. Overwriting is how you destroy the only
//! copy of a refresh token because of a bug in reading it.

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use ts_rs::TS;
use uuid::Uuid;

/// Bumped when the shape changes. A file from a version we do not know is not ours to
/// interpret and not ours to delete.
const VERSION: u32 = 1;

const FILE: &str = "openai-credentials.json";
const HOST_FILE: &str = "openai-host-id.txt";
const REGISTRATION_FILE: &str = "openai-client-id.txt";

/// Refresh this long before the token actually expires, so a request is never sent with
/// a token that dies in flight.
const REFRESH_MARGIN_SECONDS: i64 = 120;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Credentials {
    /// The client id OpenAI issued to this installation during the first sign-in. Not a
    /// secret, but it is what later sign-ins and refreshes are made against.
    pub client_id: String,
    /// The account, from the id token's `sub`.
    pub subject: String,
    pub email: Option<String>,
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// When the access token stops working, in seconds since the epoch.
    pub expires_at_unix: i64,
    /// What was actually granted, which is not always what was asked for.
    pub scopes: Vec<String>,
}

impl Credentials {
    /// Whether the access token should be refreshed before the next request.
    pub fn needs_refresh(&self, now_unix: i64) -> bool {
        now_unix + REFRESH_MARGIN_SECONDS >= self.expires_at_unix
    }

    /// Whether the grant covers using the ChatGPT plan for inference. Asking for a scope
    /// is not being given it, so this is checked before any request rather than assumed
    /// from the fact that sign-in succeeded.
    pub fn may_infer(&self) -> bool {
        self.scopes.iter().any(|s| s == super::SCOPE_INFERENCE)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Stored {
    version: u32,
    credentials: Credentials,
}

/// What the window is told. Never the tokens: the panel decides what to show, and it
/// cannot leak what it was never given.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatStatus {
    pub signed_in: bool,
    pub email: Option<String>,
    /// False when sign-in worked but the plan was not shared, which is a different
    /// problem from not being signed in and needs saying differently.
    pub can_infer: bool,
}

impl ChatStatus {
    pub fn signed_out() -> Self {
        Self {
            signed_in: false,
            email: None,
            can_infer: false,
        }
    }
}

impl From<&Credentials> for ChatStatus {
    fn from(credentials: &Credentials) -> Self {
        Self {
            signed_in: true,
            email: credentials.email.clone(),
            can_infer: credentials.may_infer(),
        }
    }
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok()
}

/// Read what is stored, or nothing.
///
/// Nothing is also the answer for a file that will not parse or that a later version
/// wrote. Both mean "this app cannot use what is here", and neither is a reason to
/// destroy it.
pub fn load(app: &AppHandle) -> Option<Credentials> {
    let path = dir(app)?.join(FILE);
    let text = fs::read_to_string(path).ok()?;
    let stored: Stored = serde_json::from_str(&text).ok()?;
    (stored.version == VERSION).then_some(stored.credentials)
}

pub fn save(app: &AppHandle, credentials: &Credentials) -> std::io::Result<()> {
    let dir = dir(app)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no config directory"))?;
    fs::create_dir_all(&dir)?;
    let path = dir.join(FILE);
    let text = serde_json::to_string_pretty(&Stored {
        version: VERSION,
        credentials: credentials.clone(),
    })?;
    write_private(&path, text.as_bytes())
}

/// Forget the tokens.
///
/// Only the tokens. The registration and the host id are not credentials and are not
/// what signing out is about: keeping them means signing back in reuses the app the
/// person already approved, instead of leaving a trail of duplicate Studio entries in
/// their ChatGPT settings.
pub fn clear(app: &AppHandle) {
    if let Some(dir) = dir(app) {
        let _ = fs::remove_file(dir.join(FILE));
    }
}

/// The client id the provider issued to this installation, if it has one yet.
pub fn registration(app: &AppHandle) -> Option<String> {
    let path = dir(app)?.join(REGISTRATION_FILE);
    let text = fs::read_to_string(path).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Record the issued client id.
///
/// Written as soon as the callback arrives, before the one-time code is exchanged,
/// because registration and the exchange can succeed and fail independently. An exchange
/// that fails after a successful registration would otherwise register a second app on
/// the next attempt.
pub fn save_registration(app: &AppHandle, client_id: &str) -> std::io::Result<()> {
    let dir = dir(app)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no config directory"))?;
    fs::create_dir_all(&dir)?;
    write_private(&dir.join(REGISTRATION_FILE), client_id.as_bytes())
}

/// This installation's identifier, made once and kept.
///
/// It must be the same on every sign-in from this machine, so it is written the first
/// time it is asked for and read thereafter. Opaque on purpose: a uuid says nothing
/// about the machine it came from.
pub fn host_id(app: &AppHandle) -> std::io::Result<String> {
    let dir = dir(app)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no config directory"))?;
    let path = dir.join(HOST_FILE);
    if let Ok(existing) = fs::read_to_string(&path) {
        let existing = existing.trim().to_string();
        if !existing.is_empty() {
            return Ok(existing);
        }
    }
    fs::create_dir_all(&dir)?;
    let fresh = format!("urn:uuid:{}", Uuid::new_v4());
    write_private(&path, fresh.as_bytes())?;
    Ok(fresh)
}

/// Write a file only its owner can read.
///
/// Two steps, because neither alone is enough. `mode` on the open options applies only
/// when the file is created, so it closes the window where a brand new file sits on disk
/// readable by anyone before being tightened. It does nothing at all to a file that
/// already exists, and this file is rewritten on every token refresh, so the mode is
/// then set again through the open handle: through the handle rather than the path so
/// that nothing can be swapped in underneath between the two.
///
/// On platforms without unix permissions this is an ordinary write, which is the best
/// that can be done there.
fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        file.write_all(bytes)
    }
    #[cfg(not(unix))]
    {
        fs::write(path, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn sample() -> Credentials {
        Credentials {
            client_id: "oaiapp_test".into(),
            subject: "user-1".into(),
            email: Some("a@example.com".into()),
            access_token: "secret".into(),
            refresh_token: Some("refresh".into()),
            expires_at_unix: 1_000_000,
            scopes: vec![super::super::SCOPE_INFERENCE.into(), "openid".into()],
        }
    }

    #[test]
    fn a_token_is_refreshed_before_it_dies_rather_than_after() {
        let credentials = sample();
        assert!(!credentials.needs_refresh(1_000_000 - REFRESH_MARGIN_SECONDS - 1));
        // Inside the margin: a request sent now might outlive the token.
        assert!(credentials.needs_refresh(1_000_000 - REFRESH_MARGIN_SECONDS));
        assert!(credentials.needs_refresh(1_000_000));
    }

    #[test]
    fn being_signed_in_is_not_the_same_as_being_allowed_to_infer() {
        let mut credentials = sample();
        assert!(credentials.may_infer());
        credentials.scopes = vec!["openid".into(), "profile".into()];
        assert!(!credentials.may_infer(), "the plan scope was not granted");
        let status = ChatStatus::from(&credentials);
        assert!(
            status.signed_in && !status.can_infer,
            "two different problems"
        );
    }

    #[test]
    fn the_window_is_never_told_a_token() {
        let status = ChatStatus::from(&sample());
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.contains("secret"), "{json}");
        assert!(!json.contains("refresh"), "{json}");
    }

    #[test]
    fn a_file_from_a_later_version_is_not_read() {
        let stored = serde_json::json!({ "version": VERSION + 1, "credentials": sample() });
        let text = serde_json::to_string(&stored).unwrap();
        let parsed: Option<Stored> = serde_json::from_str(&text).ok();
        let usable = parsed.filter(|s| s.version == VERSION);
        assert!(
            usable.is_none(),
            "a later format must not be interpreted as this one"
        );
    }

    #[test]
    fn a_private_file_is_readable_only_by_its_owner() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret.txt");
        // Pre-create it world readable, to prove the write tightens it rather than
        // leaving whatever was there.
        let mut loose = fs::File::create(&path).unwrap();
        loose.write_all(b"old").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        }

        write_private(&path, b"new").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "others can read it: {mode:o}");
        }
    }
}
