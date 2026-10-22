//! Why a call did not happen, in terms its caller can act on.
//!
//! A refusal is an answer, not a failure. The document declining an edit is the normal
//! way a caller learns what it may do, so the shape here is built for recovery: what
//! went wrong, whether trying again could ever help, what to do instead, and where the
//! document actually is.
//!
//! That third field matters more than it looks. Without it a caller's only move after a
//! refusal is to try the same thing again, which for every refusal the document issues
//! is guaranteed to fail the same way, because applying a command is deterministic.

use serde::Serialize;
use serde_json::Value;
use studio_core::edit::EditError;
use studio_core::select::SelectError;
use thiserror::Error;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ToolErrorCode {
    /// No tool by that name.
    UnknownTool,
    /// The tool exists but is not offered here.
    NotAvailable,
    /// The arguments did not fit the schema.
    BadArguments,
    /// What the caller named could not be found, or named more than one thing.
    BadSelector,
    /// The document has moved since the caller last looked.
    StaleRevision,
    /// Whatever was named is not in the document.
    NotFound,
    TrackLocked,
    WrongTrackKind,
    MediaInUse,
    InvalidArgument,
    /// The edit would have left the document invalid.
    Invalid,
    /// Something outside the document refused: a file, a tool, a running job.
    Host,
    /// Something else is using the one thing this needs. The only code worth retrying.
    Busy,
}

#[derive(Debug, Clone, PartialEq, Error, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[error("{message}")]
#[ts(export)]
pub struct ToolError {
    pub code: ToolErrorCode,
    pub message: String,
    /// What to do instead, naming the tool and the argument that would do it. Absent
    /// when there is nothing useful to say.
    pub remedy: Option<String>,
    /// Whether the same call could succeed later. False for everything the document
    /// refuses, because applying a command is deterministic: the same command against
    /// the same document is refused again. A caller that cannot tell these apart spends
    /// its turns retrying a decision that will not change.
    pub retryable: bool,
    /// Where the document actually is, so a caller holding a stale id knows the
    /// difference between "that never existed" and "the document moved, look again".
    #[ts(type = "number")]
    pub revision: u64,
    /// The underlying error, as it serializes itself.
    #[ts(type = "unknown")]
    pub details: Value,
}

impl ToolError {
    pub fn new(code: ToolErrorCode, message: impl Into<String>, revision: u64) -> Self {
        Self {
            code,
            message: message.into(),
            remedy: None,
            retryable: false,
            revision,
            details: Value::Null,
        }
    }

    pub fn with_remedy(mut self, remedy: impl Into<String>) -> Self {
        self.remedy = Some(remedy.into());
        self
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = details;
        self
    }

    pub fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }

    /// An edit the document refused.
    ///
    /// The remedies name the tool that would get the caller unstuck, because "track is
    /// locked" tells it what happened and not what to do about it.
    pub fn from_edit(error: EditError, revision: u64) -> Self {
        let code = match &error {
            EditError::NotFound { .. } => ToolErrorCode::NotFound,
            EditError::TrackLocked { .. } => ToolErrorCode::TrackLocked,
            EditError::WrongTrackKind => ToolErrorCode::WrongTrackKind,
            EditError::MediaInUse { .. } => ToolErrorCode::MediaInUse,
            EditError::InvalidArgument { .. } => ToolErrorCode::InvalidArgument,
            EditError::Invalid { .. } => ToolErrorCode::Invalid,
        };
        let remedy = match &error {
            EditError::NotFound { .. } => {
                Some("the id may be out of date; query.timeline lists what is there now")
            }
            EditError::TrackLocked { .. } => Some(
                "unlock it with track.set_state { locked: false }, or choose another track",
            ),
            EditError::WrongTrackKind => {
                Some("pictures go on video tracks and sound on audio ones; query.timeline gives each track's kind")
            }
            EditError::MediaInUse { .. } => {
                Some("remove the clips using it first; query.clips { media } lists them")
            }
            // The document's own reason is already specific, and anything added here
            // would be a worse guess than the one it gave.
            EditError::InvalidArgument { .. } => None,
            EditError::Invalid { .. } => None,
        };
        let details = serde_json::to_value(&error).unwrap_or(Value::Null);
        let mut out = Self::new(code, error.to_string(), revision).with_details(details);
        if let Some(remedy) = remedy {
            out = out.with_remedy(remedy);
        }
        out
    }

    /// Something the caller named could not be resolved.
    pub fn from_select(error: SelectError, revision: u64) -> Self {
        let details = serde_json::to_value(&error).unwrap_or(Value::Null);
        Self::new(ToolErrorCode::BadSelector, error.to_string(), revision).with_details(details)
    }

    pub fn unknown_tool(name: &str, revision: u64) -> Self {
        Self::new(
            ToolErrorCode::UnknownTool,
            format!("no tool called {name}"),
            revision,
        )
    }

    pub fn stale(expected: u64, revision: u64) -> Self {
        Self::new(
            ToolErrorCode::StaleRevision,
            format!("the document is at revision {revision}, not {expected}"),
            revision,
        )
        .with_remedy("read the document again and reissue against what is there now")
    }

    pub fn bad_arguments(why: impl Into<String>, revision: u64) -> Self {
        Self::new(ToolErrorCode::BadArguments, why, revision)
    }
}
