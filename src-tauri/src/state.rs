//! Process-wide state shared by IPC commands.

use crate::tools;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::{Arc, Mutex, MutexGuard};
use studio_core::{Editor, HistoryState, MediaId, Project};
use ts_rs::TS;

pub struct Session {
    pub editor: Editor,
    pub path: Option<PathBuf>,
}

impl Session {
    pub fn snapshot(&self) -> Snapshot {
        let project = self.editor.project().clone();
        let offline_media = project
            .media
            .iter()
            .filter(|m| !Path::new(&m.path).is_file())
            .map(|m| m.id)
            .collect();
        Snapshot {
            project,
            history: self.editor.history_state(),
            path: self.path.as_ref().map(|p| p.display().to_string()),
            offline_media,
        }
    }
}

/// Everything the UI renders, sent after every change.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Snapshot {
    pub project: Project,
    pub history: HistoryState,
    /// Where the project was last saved or opened from.
    pub path: Option<String>,
    /// Media whose file is not at its recorded path.
    pub offline_media: Vec<MediaId>,
}

/// A running render; the child is shared so a cancel command can kill it.
pub struct Job {
    pub child: Arc<Mutex<Option<Child>>>,
}

pub struct AppState {
    /// `None` until a project is made or opened. Launching into a scratch project would
    /// mean the app always has a document it cannot name, which is a thing to auto-save,
    /// a thing to prompt about discarding, and a lie the home screen would have to work
    /// around. Both reference editors open on a hub instead.
    session: Mutex<Option<Session>>,
    pub job: Mutex<Option<Job>>,
    ffmpeg: Mutex<Option<PathBuf>>,
    ffprobe: Mutex<Option<PathBuf>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            session: Mutex::new(None),
            job: Mutex::new(None),
            ffmpeg: Mutex::new(None),
            ffprobe: Mutex::new(None),
        }
    }

    /// A poisoned lock means a command panicked mid-edit. Edits are applied to a copy and
    /// swapped in atomically, so the session itself is still consistent; keep going.
    pub fn session(&self) -> MutexGuard<'_, Option<Session>> {
        self.session.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn ffmpeg(&self) -> Option<PathBuf> {
        cached(&self.ffmpeg, "ffmpeg")
    }

    pub fn ffprobe(&self) -> Option<PathBuf> {
        cached(&self.ffprobe, "ffprobe")
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Only successful lookups are cached, so installing FFmpeg while the app runs works.
fn cached(slot: &Mutex<Option<PathBuf>>, tool: &str) -> Option<PathBuf> {
    let mut guard = slot.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = tools::locate(tool);
    }
    guard.clone()
}
