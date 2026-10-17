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
///
/// Write the whole document to a temporary file beside the target, flush it to the
/// disk, then rename it over the target and flush the rename itself. Every step has to
/// be there: without the first flush the rename can land before the bytes do, and
/// without the second the directory entry can still be pointing at neither file after a
/// power cut. A failed write takes its temporary file with it.
pub fn save(project: &Project, path: &Path) -> Result<(), PersistError> {
    project.validate()?;
    let json = to_json(project)?;
    let temp = temp_path(path);
    if let Err(error) = write_all_synced(&temp, json.as_bytes()) {
        let _ = fs::remove_file(&temp);
        return Err(error.into());
    }
    fs::rename(&temp, path)?;
    sync_parent(path);
    Ok(())
}

fn write_all_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Best effort: a platform that will not open a directory for syncing (Windows) has
/// still done the rename, and the save is no worse off than before.
fn sync_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
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

    #[test]
    fn saving_again_replaces_the_file_and_leaves_no_temp() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cut.studio");
        let mut project = edited_project();
        save(&project, &path).unwrap();
        project.name = "Second pass".into();
        save(&project, &path).unwrap();
        assert_eq!(load(&path).unwrap().name, "Second pass");
        assert!(!temp_path(&path).exists());
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["cut.studio"]);
    }

    #[test]
    fn a_rejected_save_leaves_the_previous_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cut.studio");
        let good = edited_project();
        save(&good, &path).unwrap();

        // Invalid: a project keeps at least one sequence.
        let mut bad = good.clone();
        bad.sequences.clear();
        assert!(matches!(save(&bad, &path), Err(PersistError::Invalid(_))));

        assert_eq!(load(&path).unwrap(), good, "the good file is still there");
        assert!(
            !temp_path(&path).exists(),
            "and no half-written temp beside it"
        );
    }

    #[test]
    fn a_save_that_cannot_be_written_reports_it_rather_than_panicking() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("no such folder").join("cut.studio");
        assert!(matches!(
            save(&edited_project(), &path),
            Err(PersistError::Io(_))
        ));
        assert!(!path.exists());
    }

    #[test]
    fn writing_what_was_read_gives_the_same_bytes() {
        // Save, load, save: a file that goes through Studio untouched comes out
        // identical, so a project in version control does not churn on every open.
        let project = edited_project();
        let once = to_json(&project).unwrap();
        let twice = to_json(&from_json(&once).unwrap()).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn rejects_a_truncated_file() {
        let text = to_json(&edited_project()).unwrap();
        let half = &text[..text.len() / 2];
        assert!(matches!(
            from_json(half).unwrap_err(),
            PersistError::Parse(_)
        ));
    }

    #[test]
    fn rejects_a_file_that_is_not_json() {
        assert!(matches!(
            from_json("this is not a project").unwrap_err(),
            PersistError::Parse(_)
        ));
    }

    #[test]
    fn rejects_a_document_missing_a_required_field() {
        let mut value = serde_json::to_value(edited_project()).unwrap();
        value.as_object_mut().unwrap().remove("sequences");
        assert!(matches!(
            from_json(&value.to_string()).unwrap_err(),
            PersistError::Parse(_)
        ));
    }

    #[test]
    fn rejects_a_file_over_the_size_cap_without_reading_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("huge.studio");
        // Sparse, so the test costs no disk: the cap is checked from the metadata.
        let file = fs::File::create(&path).unwrap();
        file.set_len(MAX_PROJECT_BYTES + 1).unwrap();
        drop(file);
        assert!(matches!(load(&path).unwrap_err(), PersistError::TooLarge));
    }

    #[test]
    fn reports_a_missing_file_rather_than_panicking() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            load(&dir.path().join("gone.studio")).unwrap_err(),
            PersistError::Io(_)
        ));
    }

    /// Fields this build does not know are dropped rather than refused. A file from a
    /// newer build is already turned away by its `schemaVersion`, so what is left here
    /// is a hand-edited file, and opening it beats refusing it.
    #[test]
    fn ignores_a_field_it_does_not_know() {
        let project = edited_project();
        let mut value = serde_json::to_value(&project).unwrap();
        value["somethingElse"] = Value::from("from a plugin that does not exist");
        assert_eq!(
            from_json(&value.to_string()).expect("opens anyway"),
            project
        );
    }

    #[test]
    fn a_name_in_any_script_survives_the_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cut.studio");
        let mut project = edited_project();
        project.name = "Sydney ツアー 🎬 recap".into();
        project.media[0].path = "/footage/día uno/A001_C004 \"take 2\".mov".into();
        save(&project, &path).unwrap();
        assert_eq!(load(&path).unwrap(), project);
    }

    #[test]
    fn migrating_is_idempotent_at_the_current_version() {
        let value = serde_json::to_value(edited_project()).unwrap();
        let mut migrated = value.clone();
        migrate(&mut migrated, SCHEMA_VERSION);
        assert_eq!(migrated, value);
    }
}
