//! Why signing in or talking to the model did not work.
//!
//! Separate from `CommandError` because the failures are a different kind. A document
//! command fails because of the document, and there is one sensible response: show the
//! message. Sign-in fails for reasons that need different responses, and the panel can
//! only choose between them if it is told which happened: a cancelled sign-in needs no
//! message at all, an unreachable provider wants a retry, and an expired grant wants the
//! browser again.
//!
//! So `retryable` is carried explicitly rather than inferred from the code. It is the
//! difference between "this will probably work in a moment" and "doing the same thing
//! again will fail the same way", which is not something a list of codes can express on
//! its own.

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatError {
    pub code: ChatErrorCode,
    pub message: String,
    /// Whether the same action is worth trying again.
    pub retryable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ChatErrorCode {
    /// The person closed the browser, or declined. Not a problem to report.
    Cancelled,
    /// The browser would not open, so sign-in never started.
    Browser,
    /// The provider could not be reached.
    Network,
    /// The provider answered, and said no. `message` is its wording where it gave one.
    Provider,
    /// The provider's own configuration could not be verified, so nothing was sent to
    /// it. Distinct from `Network`: something answered, and it was not what it claims
    /// to be.
    Discovery,
    /// The identity in the reply could not be verified, or is a different account from
    /// the one already signed in.
    Identity,
    /// Sign-in finished without the app registration it needs.
    Registration,
    /// The stored grant is no longer good. Signing in again is the fix.
    Expired,
    /// The credentials could not be read or written.
    Storage,
    /// A sign-in is already in progress.
    Busy,
}

impl ChatError {
    pub fn new(code: ChatErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: false,
        }
    }

    /// Worth trying the same thing again: a timeout, a 5xx, a key service that is
    /// briefly unavailable.
    pub fn retryable(code: ChatErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: true,
        }
    }

    pub fn cancelled() -> Self {
        Self::new(ChatErrorCode::Cancelled, "Sign-in was cancelled.")
    }

    /// The provider's identity service is temporarily unavailable.
    ///
    /// This is deliberately not an identity failure: a key service that cannot be
    /// reached does not establish that an identity is invalid, and treating it as one
    /// would sign a person out over an outage.
    pub fn identity_unavailable() -> Self {
        Self::retryable(
            ChatErrorCode::Network,
            "ChatGPT identity verification is temporarily unavailable. Your sign-in is \
             unchanged. Try again shortly.",
        )
    }
}

impl From<std::io::Error> for ChatError {
    fn from(error: std::io::Error) -> Self {
        Self::new(ChatErrorCode::Storage, error.to_string())
    }
}

pub type ChatResult<T> = Result<T, ChatError>;
