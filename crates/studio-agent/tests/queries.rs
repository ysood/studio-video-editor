//! The read-only half of the catalogue, driven the way a caller drives it: by name,
//! with JSON in and JSON out.

use serde_json::{json, Value};
use studio_agent::{ask, catalogue, find, surface, Area, Policy, Surface, ToolErrorCode};
use studio_core::history::Editor;
use studio_core::testing::sample_edit;

/// `sample_edit` is two linked video and audio pairs at frames 12..60 and 100..124, at
/// 24 fps, so the sequence is 124 frames long.
fn session() -> Editor {
    Editor::opened(sample_edit())
}

#[test]
fn a_tool_is_named_for_its_area() {
    for tool in catalogue() {
        let (area, rest) = tool
            .name
            .split_once('.')
            .unwrap_or_else(|| panic!("{} is not area.verb_noun", tool.name));
        assert_eq!(
            area,
            tool.area.as_str(),
            "{} is in the wrong area",
            tool.name
        );
        assert!(!rest.is_empty(), "{} has no verb", tool.name);
    }
}

#[test]
fn every_tool_says_what_it_does() {
    for tool in catalogue() {
        assert!(
            !tool.description.trim().is_empty(),
            "{} has no description",
            tool.name
        );
        assert!(!tool.title.trim().is_empty(), "{} has no title", tool.name);
    }
}

#[test]
fn a_question_is_marked_as_changing_nothing() {
    for tool in catalogue().iter().filter(|t| t.area == Area::Query) {
        assert!(tool.annotations.read_only, "{} is a query", tool.name);
        assert!(
            !tool.annotations.open_world,
            "{} reads the document only",
            tool.name
        );
    }
}

#[test]
fn the_default_surface_is_short() {
    // The point of separating the catalogue from what is offered. If this ever has to
    // rise it should be because someone decided to, not because tools accumulated.
    let offered = surface(&Policy::new(Surface::External));
    assert!(
        offered.len() <= 12,
        "the default surface has grown to {}: {:?}",
        offered.len(),
        offered.iter().map(|t| &t.name).collect::<Vec<_>>()
    );
    assert!(!offered.is_empty());
}

#[test]
fn a_tool_outside_the_default_is_still_there_to_be_asked_for() {
    let offered = surface(&Policy::new(Surface::External));
    assert!(
        !offered.iter().any(|t| t.name == "query.clips"),
        "not offered by default"
    );
    assert!(find("query.clips").is_some(), "but still in the catalogue");

    let widened = Policy {
        also: vec!["query.clips".into()],
        ..Policy::new(Surface::External)
    };
    assert!(surface(&widened).iter().any(|t| t.name == "query.clips"));
}

#[test]
fn an_area_can_be_turned_on_by_name() {
    let all_queries = Policy {
        also: vec!["query".into()],
        ..Policy::new(Surface::External)
    };
    let offered = surface(&all_queries);
    let queries = catalogue().iter().filter(|t| t.area == Area::Query).count();
    assert_eq!(offered.len(), queries, "naming the area offered all of it");
}

#[test]
fn withholding_wins_over_offering() {
    let contradictory = Policy {
        also: vec!["query".into()],
        except: vec!["query.overview".into()],
        ..Policy::new(Surface::External)
    };
    let offered = surface(&contradictory);
    assert!(!offered.iter().any(|t| t.name == "query.overview"));
    assert!(offered.iter().any(|t| t.name == "query.timeline"));
}

#[test]
fn an_unknown_name_says_so_rather_than_failing_silently() {
    let session = session();
    let error = ask(&session, "query.nothing_like_this", Value::Null).unwrap_err();
    assert_eq!(error.code, ToolErrorCode::UnknownTool);
    assert!(!error.retryable);
}

#[test]
fn the_overview_names_the_sequences_to_ask_about() {
    let session = session();
    let out = ask(&session, "query.overview", Value::Null).unwrap();
    assert_eq!(out["mediaCount"], 1);
    let sequences = out["sequences"].as_array().unwrap();
    assert_eq!(sequences.len(), 1);
    assert_eq!(sequences[0]["durationTimecode"], "00:00:05:04");
    assert_eq!(sequences[0]["clipCount"], 4);
}

#[test]
fn the_timeline_comes_back_without_being_told_which_sequence() {
    let session = session();
    let out = ask(&session, "query.timeline", Value::Null).unwrap();
    // The reply names the sequence that answered, so defaulting is never silent.
    assert!(out["sequence"]["id"].is_string());
    assert_eq!(out["tracks"].as_array().unwrap().len(), 2);
}

#[test]
fn a_window_can_be_given_in_timecode() {
    let session = session();
    let out = ask(
        &session,
        "query.timeline",
        json!({ "from": "00:00:00:00", "to": "00:00:02:12" }),
    )
    .unwrap();
    let video = &out["tracks"][0]["clips"];
    assert_eq!(video.as_array().unwrap().len(), 1, "only the 12..60 clip");
    assert_eq!(video[0]["startTimecode"], "00:00:00:12");
}

#[test]
fn what_is_playing_is_asked_for_in_timecode_too() {
    let session = session();
    let out = ask(&session, "query.at", json!({ "at": "00:00:01:00" })).unwrap();
    assert_eq!(out["video"].as_array().unwrap().len(), 1);
    assert_eq!(out["audio"].as_array().unwrap().len(), 1);
    assert_eq!(out["timecode"], "00:00:01:00");
}

#[test]
fn a_point_in_a_gap_answers_with_nothing_rather_than_an_error() {
    let session = session();
    let out = ask(&session, "query.at", json!({ "at": 80 })).unwrap();
    assert!(out["video"].as_array().unwrap().is_empty());
}

#[test]
fn a_selector_that_names_nothing_is_refused_with_the_forms_that_work() {
    let session = session();
    let error = ask(&session, "query.at", json!({ "at": "whenever" })).unwrap_err();
    assert_eq!(error.code, ToolErrorCode::BadSelector);
    assert!(error.message.contains("00:00:12:00"), "{}", error.message);
    assert_eq!(error.revision, 0, "and says where the document is");
}

#[test]
fn arguments_that_do_not_fit_are_refused_before_anything_happens() {
    let session = session();
    let error = ask(&session, "query.at", json!({ "at": "0", "nonsense": 1 })).unwrap_err();
    assert_eq!(error.code, ToolErrorCode::BadArguments);
}

#[test]
fn a_missing_required_argument_is_refused() {
    let session = session();
    let error = ask(&session, "query.at", json!({})).unwrap_err();
    assert_eq!(error.code, ToolErrorCode::BadArguments);
}

#[test]
fn clips_can_be_found_by_track_named_as_a_person_would() {
    let session = session();
    let out = ask(&session, "query.clips", json!({ "tracks": ["V1"] })).unwrap();
    let found = out.as_array().unwrap();
    assert_eq!(found.len(), 2, "the picture half of each pair");
    assert_eq!(found[0]["trackId"], found[1]["trackId"]);
}

#[test]
fn conditions_narrow_rather_than_widen() {
    let session = session();
    let out = ask(
        &session,
        "query.clips",
        json!({ "tracks": ["V1"], "from": "00:00:04:00" }),
    )
    .unwrap();
    assert_eq!(out.as_array().unwrap().len(), 1);
    assert_eq!(out[0]["start"], 100);
}

#[test]
fn the_bin_says_how_many_clips_hold_each_asset() {
    let session = session();
    let out = ask(&session, "query.media", Value::Null).unwrap();
    assert_eq!(out[0]["usedByClips"], 4);
    assert_eq!(out[0]["offline"], false);
}

#[test]
fn an_input_schema_is_an_object_with_its_fields_described() {
    let tool = find("query.timeline").unwrap();
    assert_eq!(tool.input_schema["type"], "object");
    let properties = &tool.input_schema["properties"];
    for field in ["sequence", "from", "to"] {
        assert!(
            properties.get(field).is_some(),
            "{field} is not in the schema"
        );
    }
    assert!(
        properties["from"]["description"]
            .as_str()
            .unwrap_or_default()
            .contains("timecode"),
        "the doc comment did not reach the schema"
    );
}

#[test]
fn a_frame_may_be_written_as_a_number_or_as_text() {
    let session = session();
    // A caller that has just read `start: 100` out of a timeline sends the number.
    let number = ask(&session, "query.at", json!({ "at": 24 })).unwrap();
    let text = ask(&session, "query.at", json!({ "at": "24" })).unwrap();
    let timecode = ask(&session, "query.at", json!({ "at": "00:00:01:00" })).unwrap();
    assert_eq!(number, text);
    assert_eq!(number, timecode);
    assert_eq!(number["video"].as_array().unwrap().len(), 1);
}

#[test]
fn the_schema_admits_both_forms() {
    let tool = find("query.at").unwrap();
    let at = &tool.input_schema["properties"]["at"];
    // Written once and referred to, so follow the reference. A tool's schema has to
    // carry its own definitions or the reference would dangle for whoever reads it.
    let name = at["$ref"]
        .as_str()
        .unwrap_or_else(|| panic!("expected a reference: {at}"))
        .rsplit('/')
        .next()
        .unwrap();
    let definition = &tool.input_schema["$defs"][name];
    let text = serde_json::to_string(definition).unwrap();
    assert!(
        text.contains("string") && text.contains("integer"),
        "a frame should be writable either way: {text}"
    );
}

#[test]
fn a_tool_schema_resolves_on_its_own() {
    // Whoever reads these gets one tool at a time, so a schema that refers outward is
    // broken for them even though it looks right beside its siblings.
    for tool in catalogue() {
        let text = serde_json::to_string(&tool.input_schema).unwrap();
        if text.contains("$ref") {
            assert!(
                tool.input_schema.get("$defs").is_some(),
                "{} refers to definitions it does not carry",
                tool.name
            );
        }
    }
}
