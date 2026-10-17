//! Reading and writing `.studio` project files.
//!
//! A project file is the `Project` serialized as JSON. Loading parses to a JSON value
//! first, migrates it forward one schema version at a time, then deserializes and
//! validates, so an old or hand-edited file can never enter the editor in a bad state.

use crate::model::{Project, SCHEMA_VERSION};
use crate::validate::ValidationError;
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const PROJECT_EXTENSION: &str = "studio";

/// Larger files are rejected before parsing; a real project is far smaller.
pub const MAX_PROJECT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Error)]
pub enum PersistError {
    #[error("could not read or write the project file: {0}")]
    Io(#[from] std::io::Error),
    #[error("project file is larger than 64 MB")]
    TooLarge,
    #[error("project file is not valid: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("project file has no schema version")]
    MissingVersion,
    #[error("project was saved by a newer version of Studio (schema {0})")]
    FromTheFuture(u32),
    #[error("project file is not valid: {0}")]
    Invalid(#[from] ValidationError),
}

/// Parse, migrate and validate project JSON.
pub fn from_json(text: &str) -> Result<Project, PersistError> {
    let mut value: Value = serde_json::from_str(text)?;
    let version = value
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or(PersistError::MissingVersion)?;
    if version > SCHEMA_VERSION {
        return Err(PersistError::FromTheFuture(version));
    }
    migrate(&mut value, version);
    let project: Project = serde_json::from_value(value)?;
    project.validate()?;
    Ok(project)
}

pub fn to_json(project: &Project) -> Result<String, PersistError> {
    Ok(serde_json::to_string_pretty(project)?)
}

/// Upgrade `value` from `from` to `SCHEMA_VERSION`. Each step is one `match` arm that
/// rewrites the JSON in place, so old files never need old Rust types.
fn migrate(value: &mut Value, from: u32) {
    let mut version = from;
    while version < SCHEMA_VERSION {
        if version == 1 {
            migrate_v1_to_v2(value);
        }
        version += 1;
    }
    value["schemaVersion"] = Value::from(SCHEMA_VERSION);
}

/// v1 held one `sequence`; v2 holds a list of them (ADR-0004). The single sequence
/// becomes the first, so an existing project opens on the timeline it was saved with.
fn migrate_v1_to_v2(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    if let Some(sequence) = object.remove("sequence") {
        object.insert("sequences".into(), Value::Array(vec![sequence]));
    }
}

pub fn load(path: &Path) -> Result<Project, PersistError> {
    if fs::metadata(path)?.len() > MAX_PROJECT_BYTES {
        return Err(PersistError::TooLarge);
    }
    from_json(&fs::read_to_string(path)?)
}

/// Write atomically: a crash mid-save leaves the previous file intact.
pub fn save(project: &Project, path: &Path) -> Result<(), PersistError> {
    project.validate()?;
    let json = to_json(project)?;
    let temp = temp_path(path);
    {
        let mut file = fs::File::create(&temp)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
    }
    fs::rename(&temp, path)?;
    Ok(())
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{EditCommand, TrackChoice};
    use crate::history::Editor;
    use crate::testing::{media_with_av, project_with};

    fn edited_project() -> Project {
        let project = project_with(vec![media_with_av(10)]);
        let (media_id, v1, a1) = (
            project.media[0].id,
            project.first_sequence().tracks[0].id,
            project.first_sequence().tracks[1].id,
        );
        let sequence_id = project.first_sequence().id;
        let mut editor = Editor::new(project);
        editor
            .apply(&EditCommand::InsertMedia {
                sequence_id,
                media_id,
                start: 12,
                source_start: 6,
                duration: Some(48),
                video_track: TrackChoice::Track(v1),
                audio_track: TrackChoice::Track(a1),
            })
            .unwrap();
        editor.project().clone()
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cut.studio");
        let project = edited_project();
        save(&project, &path).unwrap();
        assert_eq!(load(&path).unwrap(), project);
        assert!(!temp_path(&path).exists());
    }

    #[test]
    fn rejects_file_from_newer_schema() {
        let mut value = serde_json::to_value(edited_project()).unwrap();
        value["schemaVersion"] = Value::from(SCHEMA_VERSION + 1);
        let err = from_json(&value.to_string()).unwrap_err();
        assert!(matches!(err, PersistError::FromTheFuture(_)));
    }

    #[test]
    fn rejects_structurally_invalid_document() {
        let mut value = serde_json::to_value(edited_project()).unwrap();
        // Make the video clip overlap itself by pushing a duplicate.
        let clip = value["sequences"][0]["tracks"][0]["clips"][0].clone();
        value["sequences"][0]["tracks"][0]["clips"]
            .as_array_mut()
            .unwrap()
            .push(clip);
        assert!(from_json(&value.to_string()).is_err());
    }

    #[test]
    fn migrates_a_v1_document_to_one_sequence() {
        // v1 held a single `sequence`. Rewrite a current document into that shape and
        // check it comes back as a one-element list rather than being rejected.
        let mut value = serde_json::to_value(edited_project()).unwrap();
        let object = value.as_object_mut().unwrap();
        let sequence = object.remove("sequences").unwrap()[0].clone();
        object.insert("sequence".into(), sequence);
        object.insert("schemaVersion".into(), Value::from(1));

        let project = from_json(&value.to_string()).expect("v1 document should migrate");
        assert_eq!(project.schema_version, SCHEMA_VERSION);
        assert_eq!(project.sequences.len(), 1);
        assert!(
            project.first_sequence().clips().next().is_some(),
            "clips survive"
        );
    }

    #[test]
    fn rejects_missing_version() {
        assert!(matches!(
            from_json("{}").unwrap_err(),
            PersistError::MissingVersion
        ));
    }
}
