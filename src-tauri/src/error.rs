//! The error shape every IPC command returns. The UI switches on `code` and shows
//! `message`; `detail` carries structured core errors (for example which clip overlaps).

use serde::Serialize;
use studio_core::export::render::RenderError;
use studio_core::persist::PersistError;
use studio_core::probe::ProbeError;
use studio_core::EditError;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommandError {
    pub code: ErrorCode,
    pub message: String,
    #[ts(type = "unknown")]
    pub detail: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ErrorCode {
    /// The edit was rejected; the project is unchanged.
    Edit,
    /// A path from the UI was not usable.
    BadPath,
    /// Reading or writing a project file failed.
    Persist,
    /// ffmpeg or ffprobe is missing or failed.
    Tool,
    /// The file is not media Studio can use.
    Probe,
    /// The save needs a path because the project has never been saved.
    NeedsPath,
    /// Another render or package job is still running.
    Busy,
    Io,
}

impl CommandError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
        }
    }
}

pub type CommandResult<T> = Result<T, CommandError>;

impl From<EditError> for CommandError {
    fn from(error: EditError) -> Self {
        Self {
            code: ErrorCode::Edit,
            message: error.to_string(),
            detail: serde_json::to_value(&error).ok(),
        }
    }
}

impl From<PersistError> for CommandError {
    fn from(error: PersistError) -> Self {
        Self::new(ErrorCode::Persist, error.to_string())
    }
}

impl From<ProbeError> for CommandError {
    fn from(error: ProbeError) -> Self {
        Self::new(ErrorCode::Probe, error.to_string())
    }
}

impl From<RenderError> for CommandError {
    fn from(error: RenderError) -> Self {
        Self::new(ErrorCode::Tool, error.to_string())
    }
}

impl From<std::io::Error> for CommandError {
    fn from(error: std::io::Error) -> Self {
        Self::new(ErrorCode::Io, error.to_string())
    }
}
