//! The commands as tools, driven by name with JSON, the way a caller drives them.

use serde_json::{json, Value};
use studio_agent::{act, catalogue, command_schemas, commands, find, ToolErrorCode};
use studio_core::history::Editor;
use studio_core::testing::sample_edit;

/// Two linked video and audio pairs, at frames 12..60 and 100..124, 24 fps.
fn session() -> Editor {
    Editor::opened(sample_edit())
}

#[test]
fn every_command_is_a_tool_or_is_excused() {
    let excused: Vec<&str> = commands::NOT_TOOLS.iter().map(|(name, _)| *name).collect();
    for variant in command_schemas().keys() {
        let has_tool = catalogue()
            .iter()
            .any(|t| commands::variant_of(&t.name) == Some(variant.as_str()));
        assert!(
            has_tool || excused.contains(&variant.as_str()),
            "{variant} has no tool and is not in NOT_TOOLS, so it was forgotten"
        );
    }
}

#[test]
fn nothing_is_excused_that_no_longer_exists() {
    let schemas = command_schemas();
    for (variant, _) in commands::NOT_TOOLS {
        assert!(
            schemas.contains_key(*variant),
            "{variant} is excused but is not a command any more"
        );
    }
}

#[test]
fn every_id_the_schema_mentions_is_named_by_the_table() {
    // The guard that keeps the hand-written half honest. If a command gains an id
    // argument and the table is not told, a caller would have to supply a uuid for it
    // while every other argument takes a selector.
    for tool in catalogue() {
        let Some(variant) = commands::variant_of(&tool.name) else {
            continue;
        };
        let schema = &command_schemas()[variant];
        let Some(properties) = schema["properties"].as_object() else {
            continue;
        };
        let named: Vec<&str> = commands::named_fields(&tool.name)
            .iter()
            .map(|(field, _)| *field)
            .collect();
        for (field, value) in properties {
            let refers_to_id = serde_json::to_string(value)
                .unwrap_or_default()
                .contains("Id\"");
            if refers_to_id {
                assert!(
                    named.contains(&field.as_str()),
                    "{}: {field} is an id the table does not mention",
                    tool.name
                );
            }
        }
    }
}

#[test]
fn a_command_tool_describes_itself_from_the_doc_comment() {
    let tool = find("timeline.trim_clip").unwrap();
    assert!(
        tool.description.contains("at least one frame long"),
        "{}",
        tool.description
    );
}

#[test]
fn a_selector_argument_says_what_it_accepts() {
    let tool = find("timeline.move_clip").unwrap();
    let clip = &tool.input_schema["properties"]["clipId"];
    let text = serde_json::to_string(clip).unwrap();
    assert!(text.contains("V1#3"), "the forms are not described: {text}");
    let start = &tool.input_schema["properties"]["start"];
    let text = serde_json::to_string(start).unwrap();
    assert!(
        text.contains("integer") && text.contains("string"),
        "{text}"
    );
}

#[test]
fn a_clip_can_be_moved_by_naming_it_the_way_a_person_would() {
    let mut session = session();
    let out = act(
        &mut session,
        "timeline.move_clip",
        json!({ "clipId": "V1#1", "start": "00:00:05:00" }),
        None,
    )
    .unwrap();
    assert_eq!(out.label, "Move clip");
    let moved = session.project().first_sequence().tracks[0]
        .clips
        .iter()
        .any(|c| c.start == 120);
    assert!(moved, "the clip did not land at frame 120");
}

#[test]
fn a_clip_can_be_named_by_the_frame_it_covers() {
    let mut session = session();
    act(
        &mut session,
        "timeline.trim_clip",
        json!({ "clipId": "V1@00:00:01:00", "edge": "end", "frame": 48 }),
        None,
    )
    .unwrap();
    let clip = &session.project().first_sequence().tracks[0].clips[0];
    assert_eq!(clip.end(), 48, "the clip was trimmed to the named frame");
}

#[test]
fn the_edit_that_reaches_the_document_is_the_id_form() {
    // Selectors resolve before anything is applied, so the undo history never holds an
    // entry whose meaning depends on what the document looked like at the time.
    let mut session = session();
    act(
        &mut session,
        "timeline.delete_clips",
        json!({ "clipIds": ["V1#1"] }),
        None,
    )
    .unwrap();
    assert_eq!(
        session.history_state().undo_label.as_deref(),
        Some("Delete")
    );
    // The linked sound went with it, as the command's description says.
    let sequence = session.project().first_sequence();
    assert_eq!(sequence.tracks[0].clips.len(), 1);
    assert_eq!(sequence.tracks[1].clips.len(), 1);
}

#[test]
fn a_track_can_be_chosen_with_one_word() {
    let mut session = session();
    let media = session.project().media[0].id.to_string();
    let sequence = session.project().first_sequence().id.to_string();
    let out = act(
        &mut session,
        "timeline.insert_media",
        json!({
            "sequenceId": sequence,
            "mediaId": media,
            "start": "end",
            "sourceStart": 0,
            "duration": 24,
            "videoTrack": "V1",
            "audioTrack": "skip"
        }),
        None,
    )
    .unwrap();
    assert_eq!(out.report.created_clips.len(), 1, "picture only, no sound");
    let sequence = session.project().first_sequence();
    assert_eq!(sequence.tracks[0].clips.len(), 3);
    assert_eq!(sequence.tracks[1].clips.len(), 2, "audio untouched");
}

#[test]
fn end_means_after_everything() {
    let mut session = session();
    let media = session.project().media[0].id.to_string();
    let sequence = session.project().first_sequence().id.to_string();
    act(
        &mut session,
        "timeline.insert_media",
        json!({
            "sequenceId": sequence, "mediaId": media,
            "start": "end", "sourceStart": 0, "duration": 24,
            "videoTrack": "V1", "audioTrack": "skip"
        }),
        None,
    )
    .unwrap();
    let placed = session.project().first_sequence().tracks[0]
        .clips
        .iter()
        .any(|c| c.start == 124);
    assert!(placed, "it should have landed where the timeline ended");
}

#[test]
fn an_edit_against_a_document_that_has_moved_is_refused() {
    let mut session = session();
    let stale = session.revision();
    act(
        &mut session,
        "clip.rename",
        json!({ "clipId": "V1#1", "name": "first" }),
        None,
    )
    .unwrap();

    let error = act(
        &mut session,
        "clip.rename",
        json!({ "clipId": "V1#1", "name": "again" }),
        Some(stale),
    )
    .unwrap_err();
    assert_eq!(error.code, ToolErrorCode::StaleRevision);
    assert_eq!(error.revision, session.revision(), "and says where it is");
    assert!(!error.retryable, "reissuing the same thing will not help");
}

#[test]
fn an_edit_against_the_current_revision_goes_through() {
    let mut session = session();
    let now = session.revision();
    let out = act(
        &mut session,
        "clip.rename",
        json!({ "clipId": "V1#1", "name": "first" }),
        Some(now),
    )
    .unwrap();
    assert_ne!(out.revision, now, "the document moved on");
}

#[test]
fn a_refusal_from_the_document_says_what_to_do_instead() {
    let mut session = session();
    let track = session.project().first_sequence().tracks[0].id.to_string();
    act(
        &mut session,
        "track.set_state",
        json!({ "trackId": track, "locked": true }),
        None,
    )
    .unwrap();

    let error = act(
        &mut session,
        "timeline.delete_clips",
        json!({ "clipIds": ["V1#1"] }),
        None,
    )
    .unwrap_err();
    assert_eq!(error.code, ToolErrorCode::TrackLocked);
    assert!(error.remedy.unwrap().contains("track.set_state"));
    assert!(!error.retryable);
}

#[test]
fn naming_something_that_is_not_there_is_a_selector_problem_not_an_edit_one() {
    let mut session = session();
    let error = act(
        &mut session,
        "timeline.move_clip",
        json!({ "clipId": "V9#1", "start": 0 }),
        None,
    )
    .unwrap_err();
    assert_eq!(error.code, ToolErrorCode::BadSelector);
    // Nothing was attempted, so the document has not moved.
    assert_eq!(error.revision, session.revision());
    assert!(session.history_state().undo_label.is_none());
}

#[test]
fn a_destructive_tool_says_so_and_a_safe_one_does_not() {
    assert!(
        find("timeline.delete_clips")
            .unwrap()
            .annotations
            .destructive
    );
    assert!(
        find("timeline.insert_media")
            .unwrap()
            .annotations
            .destructive
    );
    assert!(!find("timeline.trim_clip").unwrap().annotations.destructive);
    assert!(!find("clip.rename").unwrap().annotations.destructive);
    // Reaching outside the document is the exception and is worth naming: everything
    // else is a closed world, and a caller deciding whether to ask first should not be
    // told otherwise.
    let outward: Vec<String> = catalogue()
        .into_iter()
        .filter(|t| t.annotations.open_world)
        .map(|t| t.name)
        .collect();
    assert_eq!(outward, vec!["media.import".to_string()]);
}

#[test]
fn splitting_twice_at_the_same_frame_changes_nothing_the_first_did_not() {
    // Which is why it is marked idempotent.
    let mut session = session();
    act(
        &mut session,
        "timeline.split_clips",
        json!({ "clipIds": ["V1#1"], "at": 24 }),
        None,
    )
    .unwrap();
    let after_one = session.project().clone();
    let _ = act(
        &mut session,
        "timeline.split_clips",
        json!({ "clipIds": ["V1#1"], "at": 24 }),
        None,
    );
    assert_eq!(session.project(), &after_one);
}

#[test]
fn the_default_surface_is_the_editing_loop() {
    let offered: Vec<String> =
        studio_agent::surface(&studio_agent::Policy::new(studio_agent::Surface::External))
            .into_iter()
            .map(|t| t.name)
            .collect();
    for expected in [
        "query.overview",
        "query.timeline",
        "query.at",
        "timeline.insert_media",
        "timeline.move_clip",
        "timeline.trim_clip",
        "timeline.split_clips",
        "timeline.delete_clips",
        "clip.set_transform",
    ] {
        assert!(
            offered.contains(&expected.to_string()),
            "{expected} is not offered"
        );
    }
    for withheld in [
        "sequence.remove",
        "track.remove",
        "clip.unlink",
        "project.rename",
    ] {
        assert!(
            !offered.contains(&withheld.to_string()),
            "{withheld} should be off"
        );
    }
}

#[test]
fn arguments_that_are_not_an_object_are_refused_clearly() {
    let mut session = session();
    let error = act(&mut session, "clip.rename", json!("first"), None).unwrap_err();
    assert_eq!(error.code, ToolErrorCode::BadArguments);
}

#[test]
fn an_unknown_tool_is_not_an_edit_failure() {
    let mut session = session();
    let error = act(&mut session, "timeline.obliterate", Value::Null, None).unwrap_err();
    assert_eq!(error.code, ToolErrorCode::UnknownTool);
}

#[test]
fn an_edit_that_does_not_say_which_sequence_is_refused_with_where_to_look() {
    // Every command names its own sequence (ADR-0004), so that it means the same thing
    // wherever it is issued. Queries may default to the first; edits may not, because
    // reading the wrong sequence is recoverable and editing it is not.
    let mut session = session();
    let media = session.project().media[0].id.to_string();
    let error = act(
        &mut session,
        "timeline.insert_media",
        json!({
            "mediaId": media, "start": 0, "sourceStart": 0, "duration": 24,
            "videoTrack": "V1", "audioTrack": "skip"
        }),
        None,
    )
    .unwrap_err();
    assert_eq!(error.code, ToolErrorCode::BadArguments);
    assert!(error.remedy.unwrap().contains("query.overview"));
}

#[test]
fn a_sequence_can_be_named_rather_than_quoted_as_an_id() {
    let mut session = session();
    let media = session.project().media[0].id.to_string();
    let name = session.project().first_sequence().name.clone();
    act(
        &mut session,
        "timeline.insert_media",
        json!({
            "sequenceId": name, "mediaId": media, "start": "end", "sourceStart": 0,
            "duration": 24, "videoTrack": "V1", "audioTrack": "skip"
        }),
        None,
    )
    .unwrap();
    assert_eq!(session.project().first_sequence().tracks[0].clips.len(), 3);
}
