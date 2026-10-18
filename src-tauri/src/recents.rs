//! The projects this machine has opened, newest first.
//!
//! Recents are a property of the installation, not of any project, so they live beside
//! the app's own config rather than in a project file. They are also the only record of
//! where a project went once its window is closed, which is why they are kept by the
//! shell and not in web storage: clearing the webview's data should not lose the list.
//!
//! Nothing here is authoritative. A path is a hint that a file was once there, and
//! `exists` is read from disk every time the list is asked for, because a project can be
//! moved or deleted between launches.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use ts_rs::TS;

/// Enough to cover the projects anyone switches between, short enough to read at a
/// glance. Premiere shows a comparable handful on its home screen.
const KEEP: usize = 12;

/// The stored file's shape, versioned so a later format can be told apart from this one.
/// A file that does not parse is treated as empty: a lost recents list is an
/// inconvenience, and refusing to start over it would be worse.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Stored {
    version: u32,
    entries: Vec<Entry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    path: String,
    name: String,
    opened_unix_ms: i64,
}

/// One row on the home screen.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Recent {
    pub path: String,
    pub name: String,
    pub opened_unix_ms: i64,
    /// Read from disk when the list is asked for: false when the file has moved or gone.
    pub exists: bool,
}

const VERSION: u32 = 1;

fn file(app: &AppHandle) -> Option<PathBuf> {
    let dir = app.path().app_config_dir().ok()?;
    Some(dir.join("recents.json"))
}

fn read(app: &AppHandle) -> Stored {
    let Some(path) = file(app) else {
        return Stored::default();
    };
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Stored>(&text).ok())
        .filter(|stored| stored.version == VERSION)
        .unwrap_or_default()
}

/// Best effort: a recents list that cannot be written is not worth failing an open over.
fn write(app: &AppHandle, stored: &Stored) {
    let Some(path) = file(app) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(stored) {
        let _ = fs::write(path, text);
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// Put `path` at the top of the list, under the name the project goes by.
///
/// Called on every open and every save, so renaming a project or saving it somewhere new
/// updates the entry rather than adding a second one for the same file.
pub fn record(app: &AppHandle, path: &Path, name: &str) {
    let path = path.display().to_string();
    let mut stored = read(app);
    stored.version = VERSION;
    stored.entries.retain(|e| e.path != path);
    stored.entries.insert(
        0,
        Entry {
            path,
            name: name.to_string(),
            opened_unix_ms: now_ms(),
        },
    );
    stored.entries.truncate(KEEP);
    write(app, &stored);
}

/// Drop one entry, for a project the user no longer wants listed.
pub fn forget(app: &AppHandle, path: &str) {
    let mut stored = read(app);
    stored.version = VERSION;
    stored.entries.retain(|e| e.path != path);
    write(app, &stored);
}

pub fn list(app: &AppHandle) -> Vec<Recent> {
    read(app)
        .entries
        .into_iter()
        .map(|e| Recent {
            exists: Path::new(&e.path).is_file(),
            path: e.path,
            name: e.name,
            opened_unix_ms: e.opened_unix_ms,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ordering and de-duplication rules, which are the whole of the logic here and
    /// are worth pinning without reaching for an AppHandle.
    fn record_into(entries: &mut Vec<Entry>, path: &str, name: &str, at: i64) {
        entries.retain(|e| e.path != path);
        entries.insert(
            0,
            Entry {
                path: path.to_string(),
                name: name.to_string(),
                opened_unix_ms: at,
            },
        );
        entries.truncate(KEEP);
    }

    #[test]
    fn the_newest_project_comes_first() {
        let mut entries = Vec::new();
        record_into(&mut entries, "/a.studio", "A", 1);
        record_into(&mut entries, "/b.studio", "B", 2);
        assert_eq!(entries[0].path, "/b.studio");
    }

    #[test]
    fn opening_a_project_again_moves_it_up_rather_than_listing_it_twice() {
        let mut entries = Vec::new();
        record_into(&mut entries, "/a.studio", "A", 1);
        record_into(&mut entries, "/b.studio", "B", 2);
        record_into(&mut entries, "/a.studio", "A", 3);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].path, "/a.studio");
    }

    #[test]
    fn a_renamed_project_keeps_one_entry_under_its_new_name() {
        let mut entries = Vec::new();
        record_into(&mut entries, "/a.studio", "Rough cut", 1);
        record_into(&mut entries, "/a.studio", "Final", 2);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Final");
    }

    #[test]
    fn the_list_stops_growing() {
        let mut entries = Vec::new();
        for i in 0..KEEP + 5 {
            record_into(&mut entries, &format!("/{i}.studio"), "P", i as i64);
        }
        assert_eq!(entries.len(), KEEP);
    }

    #[test]
    fn a_file_that_does_not_parse_reads_as_an_empty_list() {
        let stored: Option<Stored> = serde_json::from_str("not json").ok();
        assert!(stored.is_none());
    }
}
