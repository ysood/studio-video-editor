//! What the `schema` feature promises, checked rather than assumed.
//!
//! The bet behind deriving a tool catalogue from `EditCommand` rather than writing one
//! is that the doc comments on the enum become the descriptions a caller reads. If that
//! stopped being true the catalogue would quietly ship with empty descriptions, so it is
//! asserted here.

#![cfg(feature = "schema")]

use schemars::schema_for;
use serde_json::Value;
use studio_core::edit::EditCommand;
use studio_core::query::{ClipFilter, TimelineView};

/// Every variant of the internally tagged enum, as its own branch of the `oneOf`.
fn branches() -> Vec<Value> {
    let schema = serde_json::to_value(schema_for!(EditCommand)).unwrap();
    schema
        .get("oneOf")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_else(|| panic!("EditCommand did not come out as a oneOf: {schema:#}"))
}

/// The value of the `type` tag a branch matches on, which is the tool's name.
fn tag_of(branch: &Value) -> Option<&str> {
    branch
        .pointer("/properties/type/const")
        .and_then(Value::as_str)
}

#[test]
fn every_command_is_a_branch_of_its_own() {
    let branches = branches();
    let names: Vec<&str> = branches.iter().filter_map(tag_of).collect();
    assert_eq!(names.len(), 19, "got {names:?}");
    for expected in [
        "RenameProject",
        "SetSequenceFormat",
        "AddSequence",
        "RemoveSequence",
        "RenameSequence",
        "ImportMedia",
        "RemoveMedia",
        "AddTrack",
        "RemoveTrack",
        "SetTrackState",
        "InsertMedia",
        "MoveClip",
        "TrimClip",
        "SplitClips",
        "DeleteClips",
        "SetClipsEnabled",
        "RenameClip",
        "SetClipsTransform",
        "UnlinkClips",
    ] {
        assert!(
            names.contains(&expected),
            "{expected} is missing from {names:?}"
        );
    }
}

#[test]
fn every_command_carries_the_description_written_above_it() {
    for branch in branches() {
        let name = tag_of(&branch).expect("a branch with no tag");
        let description = branch
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default();
        assert!(
            !description.trim().is_empty(),
            "{name} has no description: a tool nobody can choose correctly"
        );
    }
}

#[test]
fn a_description_is_the_doc_comment_and_not_a_restatement_of_the_name() {
    let branch = branches()
        .into_iter()
        .find(|b| tag_of(b) == Some("TrimClip"))
        .expect("TrimClip");
    let description = branch["description"].as_str().unwrap();
    assert!(
        description.contains("at least one frame long"),
        "the doc comment did not reach the schema: {description}"
    );
}

#[test]
fn an_id_is_a_uuid_shaped_string() {
    // Written once in `$defs` and referred to, rather than spelled out at each of the
    // thirty-odd places an id appears, so follow the reference to check it.
    let schema = serde_json::to_value(schema_for!(EditCommand)).unwrap();
    let branch = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| tag_of(b) == Some("RemoveMedia"))
        .expect("RemoveMedia")
        .clone();
    let reference = branch["properties"]["mediaId"]["$ref"]
        .as_str()
        .unwrap_or_else(|| panic!("mediaId is not a reference: {branch:#}"));
    let name = reference.rsplit('/').next().unwrap();
    let definition = &schema["$defs"][name];
    assert_eq!(definition["type"], "string", "got {definition:#}");
    assert_eq!(definition["format"], "uuid");
}

#[test]
fn the_tag_is_what_the_wire_format_uses() {
    // serde writes `{"type": "TrimClip", ...}`, so the schema must agree or an input
    // validated against it would be rejected by the deserializer.
    let json = serde_json::to_value(EditCommand::RemoveSequence {
        sequence_id: studio_core::SequenceId::new(),
    })
    .unwrap();
    assert_eq!(json["type"], "RemoveSequence");
}

#[test]
fn referenced_types_are_defined_rather_than_dangling() {
    let schema = serde_json::to_value(schema_for!(EditCommand)).unwrap();
    let defs = schema
        .get("$defs")
        .and_then(Value::as_object)
        .expect("no $defs, so every $ref in the branches is dangling");
    for expected in ["TrackChoice", "SequenceFormat", "MediaAsset", "ClipEdge"] {
        assert!(defs.contains_key(expected), "{expected} is not defined");
    }
}

#[test]
fn query_types_have_schemas_too() {
    // Inputs and outputs both: a caller needs the shape of what it sends and of what it
    // gets back.
    let filter = serde_json::to_value(schema_for!(ClipFilter)).unwrap();
    assert!(
        filter["properties"].get("nameContains").is_some(),
        "{filter:#}"
    );
    let view = serde_json::to_value(schema_for!(TimelineView)).unwrap();
    assert!(view["properties"].get("tracks").is_some(), "{view:#}");
}
