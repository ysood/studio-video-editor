//! What an edit did, including the parts nobody asked for.

use serde_json::json;
use studio_agent::delta::Change;
use studio_agent::{act, import, Host, NoHost};
use studio_core::history::Editor;
use studio_core::testing::{media_with_av, sample_edit};

/// Two linked video and audio pairs, frames 12..60 and 100..124, at 24 fps.
fn session() -> Editor {
    Editor::opened(sample_edit())
}

#[test]
fn an_insert_reports_the_clips_it_cleared_out_of_the_way() {
    // The whole reason this exists. EditReport says one clip was created. It does not
    // say that putting it there removed the clip underneath, and a caller believing the
    // report would carry a wrong picture of the timeline into every later decision.
    let mut session = session();
    let media = session.project().media[0].id.to_string();
    let sequence = session.project().first_sequence().id.to_string();
    let doomed = session.project().first_sequence().tracks[0].clips[0].id;

    let out = act(
        &mut session,
        "timeline.insert_media",
        json!({
            "sequenceId": sequence, "mediaId": media,
            "start": 12, "sourceStart": 0, "duration": 48,
            "videoTrack": "V1", "audioTrack": "skip"
        }),
        None,
    )
    .unwrap();

    assert_eq!(
        out.report.created_clips.len(),
        1,
        "the command reports one creation"
    );
    assert_eq!(out.delta.clips_created.len(), 1);
    assert!(
        out.delta.clips_removed.iter().any(|c| c.id == doomed),
        "the clip it overwrote is not in the delta: {:?}",
        out.delta
    );
}

#[test]
fn a_removed_clip_is_described_because_it_cannot_be_looked_up_any_more() {
    let mut session = session();
    let clip = session.project().first_sequence().tracks[0].clips[0].clone();
    let out = act(
        &mut session,
        "timeline.delete_clips",
        json!({ "clipIds": [clip.id.to_string()] }),
        None,
    )
    .unwrap();
    let note = out
        .delta
        .clips_removed
        .iter()
        .find(|c| c.id == clip.id)
        .expect("the clip asked for");
    assert_eq!(note.name, clip.name);
    assert_eq!(note.start, clip.start);
    assert_eq!(
        out.delta.clips_removed.len(),
        2,
        "the linked sound went too"
    );
}

#[test]
fn a_caller_can_tell_whether_an_edit_reached_past_what_it_named() {
    let mut session = session();
    let clip = session.project().first_sequence().tracks[0].clips[0].id;

    // Renaming touches only what was named.
    let out = act(
        &mut session,
        "clip.rename",
        json!({ "clipId": clip.to_string(), "name": "opening" }),
        None,
    )
    .unwrap();
    assert!(!out.delta.touched_beyond(&[clip]));

    // Deleting reaches the link group, which the caller did not name.
    let clip = session.project().first_sequence().tracks[0].clips[0].id;
    let out = act(
        &mut session,
        "timeline.delete_clips",
        json!({ "clipIds": [clip.to_string()] }),
        None,
    )
    .unwrap();
    assert!(out.delta.touched_beyond(&[clip]), "the sound went with it");
}

#[test]
fn changes_are_named_rather_than_only_counted() {
    let mut session = session();
    let clip = session.project().first_sequence().tracks[0].clips[0].id;
    let out = act(
        &mut session,
        "timeline.move_clip",
        json!({ "clipId": clip.to_string(), "start": 200 }),
        None,
    )
    .unwrap();
    let changed = out
        .delta
        .clips_changed
        .iter()
        .find(|c| c.clip.id == clip)
        .expect("the clip that moved");
    assert!(changed.changes.contains(&Change::Moved));
    assert!(!changed.changes.contains(&Change::Trimmed));
}

#[test]
fn a_trim_reads_as_a_trim() {
    let mut session = session();
    let clip = session.project().first_sequence().tracks[0].clips[0].id;
    let out = act(
        &mut session,
        "timeline.trim_clip",
        json!({ "clipId": clip.to_string(), "edge": "end", "frame": 48 }),
        None,
    )
    .unwrap();
    let changed = &out.delta.clips_changed[0];
    assert!(changed.changes.contains(&Change::Trimmed));
    assert!(!changed.changes.contains(&Change::Moved), "the start held");
}

#[test]
fn unlinking_is_called_out_on_its_own() {
    let mut session = session();
    let clip = session.project().first_sequence().tracks[0].clips[0].id;
    let out = act(
        &mut session,
        "clip.unlink",
        json!({ "clipIds": [clip.to_string()] }),
        None,
    )
    .unwrap();
    assert!(out
        .delta
        .clips_changed
        .iter()
        .all(|c| c.changes.contains(&Change::Unlinked)));
    assert_eq!(out.delta.clips_changed.len(), 2, "both halves of the pair");
}

#[test]
fn removing_a_track_reports_the_clips_that_went_with_it() {
    let mut session = session();
    let track = session.project().first_sequence().tracks[0].id;
    let out = act(
        &mut session,
        "track.remove",
        json!({ "trackId": track.to_string() }),
        None,
    )
    .unwrap();
    assert_eq!(out.delta.tracks_removed, vec![track]);
    assert_eq!(out.delta.clips_removed.len(), 2, "the clips on it");
}

/// A host that answers with whatever it is told to, standing in for one that reads
/// files. Probing is the only part a host owns, so this is the whole of it.
struct Pretend {
    assets: Vec<studio_core::model::MediaAsset>,
    failures: Vec<studio_agent::host::ImportFailure>,
}

impl Host for Pretend {
    fn probe(
        &self,
        _paths: &[String],
    ) -> Result<
        (
            Vec<studio_core::model::MediaAsset>,
            Vec<studio_agent::host::ImportFailure>,
        ),
        studio_agent::ToolError,
    > {
        Ok((self.assets.clone(), self.failures.clone()))
    }
}

#[test]
fn importing_adds_what_could_be_read_and_reports_what_could_not() {
    let mut session = Editor::opened(studio_core::testing::project_with(vec![]));
    let host = Pretend {
        assets: vec![media_with_av(5)],
        failures: vec![studio_agent::host::ImportFailure {
            path: "/notes.txt".into(),
            reason: "not a media file".into(),
        }],
    };
    let out = import(
        &mut session,
        &host,
        &["/a.mov".into(), "/notes.txt".into()],
        None,
    )
    .unwrap();
    assert_eq!(out.failed.len(), 1, "the one that could not be read");
    let outcome = out.outcome.expect("the readable one was added");
    assert_eq!(outcome.delta.media_added, 1);
    assert_eq!(session.project().media.len(), 1);
}

#[test]
fn an_import_where_nothing_could_be_read_changes_nothing() {
    let mut session = Editor::opened(studio_core::testing::project_with(vec![]));
    let host = Pretend {
        assets: vec![],
        failures: vec![studio_agent::host::ImportFailure {
            path: "/notes.txt".into(),
            reason: "not a media file".into(),
        }],
    };
    let before = session.revision();
    let out = import(&mut session, &host, &["/notes.txt".into()], None).unwrap();
    assert!(out.outcome.is_none(), "nothing was applied");
    assert_eq!(out.failed.len(), 1);
    assert_eq!(session.revision(), before, "the document did not move");
}

#[test]
fn a_host_that_cannot_reach_outside_refuses_by_name() {
    let mut session = session();
    let error = import(&mut session, &NoHost, &["/a.mov".into()], None).unwrap_err();
    assert_eq!(error.code, studio_agent::ToolErrorCode::NotAvailable);
    assert!(error.message.contains("media.import"), "{}", error.message);
}

#[test]
fn a_command_that_changes_nothing_reports_an_empty_delta() {
    let mut session = session();
    let clip = session.project().first_sequence().tracks[0].clips[0].clone();
    let out = act(
        &mut session,
        "clip.rename",
        json!({ "clipId": clip.id.to_string(), "name": clip.name }),
        None,
    )
    .unwrap();
    assert!(out.delta.is_empty(), "{:?}", out.delta);
}
