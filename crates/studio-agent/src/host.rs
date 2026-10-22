//! The few things that cannot be done by reading the document.
//!
//! Almost every tool is a pure function of the project: it reads, resolves what the
//! caller named into ids, and returns a command. A handful are not, because they have
//! to read a file, run ffprobe or start a render, and that work belongs to whoever is
//! hosting rather than to a catalogue of document capability.
//!
//! Every method refuses by default. A host implements what it can do and says nothing
//! about the rest, so a process with no ffmpeg is not obliged to pretend, and the
//! caller gets a refusal naming the tool rather than a failure further in.

use crate::error::{ToolError, ToolErrorCode};
use studio_core::model::MediaAsset;

/// Why one file could not be brought in, so a caller is told which of several failed
/// rather than only that something did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportFailure {
    pub path: String,
    pub reason: String,
}

/// What a host can do beyond the document.
pub trait Host {
    /// Inspect files and describe them, without adding anything to the document.
    ///
    /// Probing and adding are separate on purpose: adding is an ordinary command that
    /// goes through the same validation and undo as every other, so the only part a
    /// host owns is finding out what is actually on disk.
    fn probe(&self, _paths: &[String]) -> Result<(Vec<MediaAsset>, Vec<ImportFailure>), ToolError> {
        Err(unsupported("media.import"))
    }

    /// Which media is not where the document says it is. A host that cannot look says
    /// nothing is missing rather than guessing.
    fn offline(&self) -> Vec<studio_core::ids::MediaId> {
        Vec::new()
    }
}

/// A host that can do none of it, for a caller that only reads and edits.
pub struct NoHost;

impl Host for NoHost {}

pub fn unsupported(tool: &str) -> ToolError {
    ToolError::new(
        ToolErrorCode::NotAvailable,
        format!("{tool} is not available here"),
        0,
    )
    .with_remedy("this caller cannot reach outside the document")
}
