//! The commands, as tools.
//!
//! One tool per command, named for the command, with the schema and the description
//! taken from the enum rather than written again. A command added to the document
//! arrives here with its tool already made, which is the whole reason for deriving
//! rather than listing.
//!
//! What cannot be derived is which arguments name part of the document. The schema says
//! `trackId` is a uuid, and a caller that has not read the timeline yet does not have
//! one; it has `V1`. So there is a table below of which fields are selectors and what
//! they select. It is the only hand-written part, it is short, and a test asserts it
//! covers every id the schema mentions, so the two cannot drift apart.

use crate::error::{ToolError, ToolErrorCode};
use crate::session::Session;
use crate::tool::{Annotations, Area, Surface, Tool};
use serde_json::{json, Map, Value};
use studio_core::edit::EditCommand;
use studio_core::model::Sequence;
use studio_core::select;

/// What an argument names, when it names part of the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Names {
    Sequence,
    Track,
    /// `InsertMedia`'s choice of track: a track, or the words `new` or `skip`.
    TrackOrWord,
    Clip,
    Clips,
    Media,
    /// A frame, a timecode, or `start` / `end`.
    Time,
}

/// One command's tool: what it is called, where it belongs, how it behaves, and which
/// of its arguments name things.
struct Spec {
    variant: &'static str,
    name: &'static str,
    area: Area,
    title: &'static str,
    annotations: Annotations,
    /// Offered without anyone asking. The ordinary editing loop only.
    default: bool,
    /// Argument name to what it names. Anything absent is taken as the schema says.
    names: &'static [(&'static str, Names)],
}

/// Every command that is a tool.
///
/// `ImportMedia` is not here. It takes assets already probed, down to their size and
/// modification time, which a caller cannot know and should not be able to assert: a
/// made-up asset is a false claim about what is on disk. Bringing media in needs to
/// read the file, so it belongs with the tools that reach outside the document.
const SPECS: &[Spec] = &[
    Spec {
        variant: "RenameProject",
        name: "project.rename",
        area: Area::Project,
        title: "Rename the project",
        annotations: Annotations::SETS,
        default: false,
        names: &[],
    },
    Spec {
        variant: "SetSequenceFormat",
        name: "sequence.set_format",
        area: Area::Sequence,
        title: "Change sequence settings",
        // Changing the frame rate is refused unless the sequence is empty, but the size
        // can change under clips that were framed for the old one.
        annotations: Annotations::REMOVES,
        default: false,
        names: &[("sequenceId", Names::Sequence)],
    },
    Spec {
        variant: "AddSequence",
        name: "sequence.add",
        area: Area::Sequence,
        title: "Add a sequence",
        annotations: Annotations::ADDS,
        default: false,
        names: &[],
    },
    Spec {
        variant: "RemoveSequence",
        name: "sequence.remove",
        area: Area::Sequence,
        title: "Remove a sequence",
        annotations: Annotations::REMOVES,
        default: false,
        names: &[("sequenceId", Names::Sequence)],
    },
    Spec {
        variant: "RenameSequence",
        name: "sequence.rename",
        area: Area::Sequence,
        title: "Rename a sequence",
        annotations: Annotations::SETS,
        default: false,
        names: &[("sequenceId", Names::Sequence)],
    },
    Spec {
        variant: "RemoveMedia",
        name: "media.remove",
        area: Area::Media,
        title: "Remove media from the bin",
        annotations: Annotations::REMOVES,
        default: false,
        names: &[("mediaId", Names::Media)],
    },
    Spec {
        variant: "AddTrack",
        name: "track.add",
        area: Area::Track,
        title: "Add a track",
        annotations: Annotations::ADDS,
        default: false,
        names: &[("sequenceId", Names::Sequence)],
    },
    Spec {
        variant: "RemoveTrack",
        name: "track.remove",
        area: Area::Track,
        title: "Remove a track",
        annotations: Annotations::REMOVES,
        default: false,
        names: &[("trackId", Names::Track)],
    },
    Spec {
        variant: "SetTrackState",
        name: "track.set_state",
        area: Area::Track,
        title: "Enable or lock a track",
        annotations: Annotations::SETS,
        default: false,
        names: &[("trackId", Names::Track)],
    },
    Spec {
        variant: "InsertMedia",
        name: "timeline.insert_media",
        area: Area::Timeline,
        // The most destructive tool here, and the description says so: what is under
        // the new clip is trimmed, split or removed to make room.
        title: "Put media on the timeline",
        annotations: Annotations::REMOVES.not_idempotent(),
        default: true,
        names: &[
            ("sequenceId", Names::Sequence),
            ("mediaId", Names::Media),
            ("start", Names::Time),
            ("sourceStart", Names::Time),
            ("videoTrack", Names::TrackOrWord),
            ("audioTrack", Names::TrackOrWord),
        ],
    },
    Spec {
        variant: "MoveClip",
        name: "timeline.move_clip",
        area: Area::Timeline,
        title: "Move a clip",
        // Destructive because the place it lands is cleared the same way.
        annotations: Annotations::REMOVES,
        default: true,
        names: &[
            ("clipId", Names::Clip),
            ("start", Names::Time),
            ("trackId", Names::Track),
        ],
    },
    Spec {
        variant: "TrimClip",
        name: "timeline.trim_clip",
        area: Area::Timeline,
        title: "Trim a clip",
        // Not destructive: the new edge is clamped to the neighbours, so trimming can
        // never take another clip with it.
        annotations: Annotations::SETS,
        default: true,
        names: &[("clipId", Names::Clip), ("frame", Names::Time)],
    },
    Spec {
        variant: "SplitClips",
        name: "timeline.split_clips",
        area: Area::Timeline,
        title: "Split clips",
        // Idempotent for a reason worth knowing: after a split at a frame, no clip
        // spans it any more, so asking again does nothing.
        annotations: Annotations::SETS,
        default: true,
        names: &[("clipIds", Names::Clips), ("at", Names::Time)],
    },
    Spec {
        variant: "DeleteClips",
        name: "timeline.delete_clips",
        area: Area::Timeline,
        title: "Delete clips",
        annotations: Annotations::REMOVES,
        default: true,
        names: &[("clipIds", Names::Clips)],
    },
    Spec {
        variant: "SetClipsEnabled",
        name: "clip.set_enabled",
        area: Area::Clip,
        title: "Enable or disable clips",
        annotations: Annotations::SETS,
        default: false,
        names: &[("clipIds", Names::Clips)],
    },
    Spec {
        variant: "RenameClip",
        name: "clip.rename",
        area: Area::Clip,
        title: "Rename a clip",
        annotations: Annotations::SETS,
        default: false,
        names: &[("clipId", Names::Clip)],
    },
    Spec {
        variant: "SetClipsTransform",
        name: "clip.set_transform",
        area: Area::Clip,
        title: "Move, scale, turn or fade a clip",
        // Values are clamped and rotation wraps, so what is stored is already the
        // canonical form of what was asked for and asking twice changes nothing.
        annotations: Annotations::SETS,
        default: true,
        names: &[("clipIds", Names::Clips)],
    },
    Spec {
        variant: "UnlinkClips",
        name: "clip.unlink",
        area: Area::Clip,
        title: "Unlink picture from sound",
        // Destructive: it destroys a relation, and there is no command that makes one.
        annotations: Annotations::REMOVES,
        default: false,
        names: &[("clipIds", Names::Clips)],
    },
];

fn spec(name: &str) -> Option<&'static Spec> {
    SPECS.iter().find(|s| s.name == name)
}

/// The schema for a selector argument: what it accepts, said in the schema rather than
/// only in prose, so a caller validating against it is not told that `V1` is wrong.
fn selector_schema(names: Names) -> Value {
    let (kinds, description): (Value, &str) = match names {
        Names::Sequence => (json!(["string"]), "A sequence, by id or by name."),
        Names::Track => (
            json!(["string"]),
            "A track, by id, by name such as V1, or by kind and position such as video:1.",
        ),
        Names::TrackOrWord => (
            json!(["string"]),
            "A track, by id or by name such as V1; or `new` to add one; or `skip` to leave \
             this stream off the timeline.",
        ),
        Names::Clip => (
            json!(["string"]),
            "A clip, by id, by the track and time it covers such as V1@00:00:12:00, or by \
             its position along a track such as V1#3.",
        ),
        Names::Clips => (
            json!(["string"]),
            "Clips, each by id, by V1@00:00:12:00 or by V1#3.",
        ),
        Names::Media => (json!(["string"]), "Media in the bin, by id."),
        Names::Time => (
            json!(["string", "integer"]),
            "A frame number, a timecode such as 00:00:12:00, or `start` / `end`.",
        ),
    };
    let base = json!({ "type": kinds, "description": description });
    if names == Names::Clips {
        return json!({ "type": "array", "items": base, "description": "Clips, each named as a selector." });
    }
    base
}

/// The derived schema with its selector arguments widened to what they really accept.
fn widen(mut schema: Value, names: &[(&str, Names)]) -> Value {
    let Some(properties) = schema.get_mut("properties").and_then(Value::as_object_mut) else {
        return schema;
    };
    for (field, kind) in names {
        if let Some(slot) = properties.get_mut(*field) {
            // Keep whatever the doc comment said about the field; the selector text
            // explains the forms, the doc comment explains the meaning.
            let inherited = slot
                .get("description")
                .and_then(Value::as_str)
                .map(str::to_string);
            let mut widened = selector_schema(*kind);
            if let (Some(inherited), Some(object)) = (inherited, widened.as_object_mut()) {
                let own = object
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                object.insert("description".into(), json!(format!("{inherited} {own}")));
            }
            *slot = widened;
        }
    }
    schema
}

/// Every command tool.
pub fn tools() -> Vec<Tool> {
    let schemas = crate::command_schemas();
    SPECS
        .iter()
        .filter_map(|spec| {
            let schema = schemas.get(spec.variant)?;
            let description = schema
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            Some(Tool {
                name: spec.name.to_string(),
                area: spec.area,
                title: spec.title.to_string(),
                description,
                input_schema: widen(schema.clone(), spec.names),
                annotations: spec.annotations,
                // Everything that changes the document is offered in the app. Whether a
                // caller in another process gets it is the policy's business, not the
                // tool's, except where it is unsafe in principle, and none of these are.
                surfaces: vec![Surface::InApp, Surface::External],
                in_default_surface: spec.default,
            })
        })
        .collect()
}

/// What a caller wrote, turned into the command the document takes.
///
/// Selectors are resolved here and nowhere later, so what reaches `apply` and the undo
/// stack is always the id form. A selector can therefore never leave an entry in the
/// history that nobody can read.
pub fn to_command(
    session: &dyn Session,
    name: &str,
    arguments: Value,
) -> Result<EditCommand, ToolError> {
    let revision = session.revision();
    let spec = spec(name).ok_or_else(|| ToolError::unknown_tool(name, revision))?;
    let mut object = match arguments {
        Value::Null => Map::new(),
        Value::Object(map) => map,
        other => {
            return Err(ToolError::bad_arguments(
                format!("arguments must be an object, not {other}"),
                revision,
            ))
        }
    };

    // The sequence a selector is read against. Most commands name one; the rest are
    // about a clip or track, which the first sequence holding it answers for.
    let sequence = sequence_for(session, &object, spec)?;

    for (field, kind) in spec.names {
        let Some(value) = object.get(*field).cloned() else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        let resolved = resolve(session, &sequence, *kind, value)?;
        object.insert((*field).to_string(), resolved);
    }

    object.insert("type".into(), json!(spec.variant));
    serde_json::from_value(Value::Object(object)).map_err(|e| {
        let message = e.to_string();
        let error = ToolError::bad_arguments(message.clone(), revision);
        // A command names its own sequence on purpose (ADR-0004), so that it means the
        // same thing wherever it is issued and a second writer cannot edit the one
        // nobody was looking at. That is right and it is also the argument a caller
        // forgets first, so the refusal says where to find it.
        if message.contains("sequenceId") {
            return error.with_remedy(
                "name the sequence; query.overview lists them with their ids and names",
            );
        }
        if message.contains("missing field") {
            return error.with_remedy("the tool's input schema lists what is required");
        }
        error
    })
}

/// Which sequence this command's selectors are read against.
fn sequence_for(
    session: &dyn Session,
    object: &Map<String, Value>,
    spec: &Spec,
) -> Result<Sequence, ToolError> {
    let project = session.project();
    let revision = session.revision();
    if let Some(named) = object.get("sequenceId").and_then(as_text) {
        let id =
            select::sequence(project, &named).map_err(|e| ToolError::from_select(e, revision))?;
        if let Some(found) = project.sequence(id) {
            return Ok(found.clone());
        }
    }
    // A clip or a track names its own sequence, so find the one that holds it rather
    // than making the caller say twice.
    for (field, kind) in spec.names {
        let Some(text) = object.get(*field).and_then(as_text) else {
            continue;
        };
        let found =
            match kind {
                Names::Track | Names::TrackOrWord => studio_core::ids::TrackId::parse(&text)
                    .and_then(|id| project.sequence_of_track(id)),
                Names::Clip | Names::Clips => studio_core::ids::ClipId::parse(&text)
                    .and_then(|id| project.sequence_of_clip(id)),
                _ => None,
            };
        if let Some(sequence) = found {
            return Ok(sequence.clone());
        }
    }
    Ok(project.first_sequence().clone())
}

/// A selector as text, whether it was written as a string or as a number, and taking
/// the first entry of a list so a list of clips can say which sequence it is in.
fn as_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Array(items) => items.first().and_then(as_text),
        _ => None,
    }
}

fn resolve(
    session: &dyn Session,
    sequence: &Sequence,
    kind: Names,
    value: Value,
) -> Result<Value, ToolError> {
    let revision = session.revision();
    let fail = |e: studio_core::select::SelectError| ToolError::from_select(e, revision);
    match kind {
        Names::Clips => {
            let Value::Array(items) = value else {
                return Err(ToolError::bad_arguments(
                    "expected a list of clips",
                    revision,
                ));
            };
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(resolve(session, sequence, Names::Clip, item)?);
            }
            Ok(Value::Array(out))
        }
        Names::Clip => {
            let text = as_text(&value).ok_or_else(|| {
                ToolError::bad_arguments("expected a clip, written as text", revision)
            })?;
            Ok(json!(select::clip(sequence, &text)
                .map_err(fail)?
                .to_string()))
        }
        Names::Track => {
            let text = as_text(&value).ok_or_else(|| {
                ToolError::bad_arguments("expected a track, written as text", revision)
            })?;
            Ok(json!(select::track(sequence, &text)
                .map_err(fail)?
                .to_string()))
        }
        Names::TrackOrWord => {
            let text = as_text(&value).ok_or_else(|| {
                ToolError::bad_arguments("expected a track, new, or skip", revision)
            })?;
            // `TrackChoice` is tagged, so the words and a track are three shapes of the
            // same argument, and a caller writes one string for all three.
            match text.trim().to_ascii_lowercase().as_str() {
                "new" => Ok(json!({ "type": "new" })),
                "skip" => Ok(json!({ "type": "skip" })),
                _ => {
                    let id = select::track(sequence, &text).map_err(fail)?;
                    Ok(json!({ "type": "track", "trackId": id.to_string() }))
                }
            }
        }
        Names::Sequence => {
            let text = as_text(&value).ok_or_else(|| {
                ToolError::bad_arguments("expected a sequence, written as text", revision)
            })?;
            Ok(json!(select::sequence(session.project(), &text)
                .map_err(fail)?
                .to_string()))
        }
        Names::Media => {
            let text = as_text(&value).ok_or_else(|| {
                ToolError::bad_arguments("expected media, written as text", revision)
            })?;
            studio_core::ids::MediaId::parse(&text)
                .filter(|id| session.project().media(*id).is_some())
                .map(|id| json!(id.to_string()))
                .ok_or_else(|| {
                    ToolError::new(
                        ToolErrorCode::BadSelector,
                        format!("no media called {text}"),
                        revision,
                    )
                    .with_remedy("query.media lists the bin with each asset's id")
                })
        }
        Names::Time => {
            let text = as_text(&value)
                .ok_or_else(|| ToolError::bad_arguments("expected a time", revision))?;
            Ok(json!(select::time(sequence, &text).map_err(fail)?))
        }
    }
}

/// Which fields of a command name part of the document, for the test that nothing was
/// forgotten.
pub fn named_fields(tool: &str) -> &'static [(&'static str, Names)] {
    spec(tool).map_or(&[], |s| s.names)
}

/// Which command a tool issues, for the test that every command has one.
pub fn variant_of(tool: &str) -> Option<&'static str> {
    spec(tool).map(|s| s.variant)
}

/// Every command that is deliberately not a tool here, and why, so the exhaustiveness
/// test can tell "left out on purpose" from "forgotten".
pub const NOT_TOOLS: &[(&str, &str)] = &[(
    "ImportMedia",
    "takes assets already probed; importing has to read the file, so it is a tool that \
     reaches outside the document",
)];
