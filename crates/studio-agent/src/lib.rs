//! Studio's document capability, as a catalogue of tools, independent of how they are
//! carried.
//!
//! The commands that change the document already exist and are already the only way it
//! changes, so this crate adds no capability. What it adds is a list: the names, the
//! argument shapes, what each one refuses, and whether it may be offered to a caller in
//! another process. Everything here is plain data and pure functions, so the same
//! catalogue serves a pane inside the app, a server in front of it, and a process with
//! no window at all.
//!
//! Three ideas carry the design.
//!
//! The catalogue is derived, not written. Tool names, argument schemas and descriptions
//! come from the command enum itself, so a command added later arrives with a tool
//! already attached rather than needing one remembered.
//!
//! Everything is in the catalogue; only a little is offered. A long list of tools is the
//! usual way a caller gets worse rather than better, because every entry costs it
//! attention on every turn. [`catalogue`] holds all of them and [`surface`] decides
//! which are put in front of anyone.
//!
//! A refusal is an answer. The document turning an edit down is how a caller learns what
//! it may do, so errors carry what to do instead and whether trying again could help.

pub mod commands;
pub mod delta;
pub mod error;
pub mod host;
pub mod queries;
pub mod schema;
pub mod session;
pub mod tool;

use schemars::schema_for;
use serde_json::Value;
use std::collections::BTreeMap;

pub use delta::{delta, Delta};
pub use error::{ToolError, ToolErrorCode};
pub use host::{Host, NoHost};
pub use session::Session;
pub use tool::{Annotations, Area, Surface, Tool};

/// Which tools a caller is offered.
///
/// The default is deliberately short. Everything not in it is still in the catalogue and
/// still reachable by name, so widening this is a decision someone makes rather than a
/// thing they have to build.
#[derive(Debug, Clone, PartialEq)]
pub struct Policy {
    pub surface: Surface,
    /// Names beyond the default set. An area's name on its own means all of it.
    pub also: Vec<String>,
    /// Names to withhold, which wins over everything else.
    pub except: Vec<String>,
}

impl Policy {
    pub fn new(surface: Surface) -> Self {
        Self {
            surface,
            also: Vec::new(),
            except: Vec::new(),
        }
    }

    fn allows(&self, tool: &Tool) -> bool {
        if !tool.available_on(self.surface) {
            return false;
        }
        let named_in = |list: &[String]| {
            list.iter()
                .any(|n| n == &tool.name || n == tool.area.as_str())
        };
        if named_in(&self.except) {
            return false;
        }
        tool.in_default_surface || named_in(&self.also)
    }
}

/// Every tool there is, in name order.
pub fn catalogue() -> Vec<Tool> {
    let mut tools = queries();
    tools.extend(commands::tools());
    tools.extend(host_tools());
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    tools
}

/// The tools a caller is offered under this policy.
pub fn surface(policy: &Policy) -> Vec<Tool> {
    catalogue()
        .into_iter()
        .filter(|tool| policy.allows(tool))
        .collect()
}

/// A tool by name, whether or not it is offered. Dispatch looks here and the policy is
/// checked separately, so "that tool is not offered to you" and "there is no such tool"
/// stay different answers.
pub fn find(name: &str) -> Option<Tool> {
    catalogue().into_iter().find(|t| t.name == name)
}

fn schema_of<T: schemars::JsonSchema>() -> Value {
    serde_json::to_value(schema_for!(T)).unwrap_or_else(|_| schema::nothing())
}

fn host_tools() -> Vec<Tool> {
    vec![Tool {
        name: "media.import".into(),
        area: Area::Media,
        title: "Bring media into the bin".into(),
        description: "Read these files and add what is usable to the bin. A file that \
            cannot be read is reported and the rest are still added. A path already in the \
            bin is skipped rather than added twice, so running this again after a failure \
            is safe."
            .into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Absolute paths to media files."
                }
            },
            "required": ["paths"]
        }),
        // Idempotent by path, which is worth advertising: a caller retrying after a
        // partial failure does not end up with the bin twice over.
        annotations: Annotations::SETS.reaching_outside(),
        surfaces: vec![Surface::InApp, Surface::External],
        in_default_surface: true,
    }]
}

fn queries() -> Vec<Tool> {
    vec![
        Tool {
            name: "query.overview".into(),
            area: Area::Query,
            title: "Project overview".into(),
            description: "The project and its sequences, with each sequence's length and \
                how many tracks and clips it has, and no clips listed. The place to start: \
                it names the sequences every other tool can then be pointed at."
                .into(),
            input_schema: schema::nothing(),
            annotations: Annotations::READ,
            surfaces: vec![Surface::InApp, Surface::External],
            in_default_surface: true,
        },
        Tool {
            name: "query.timeline".into(),
            area: Area::Query,
            title: "Read the timeline".into(),
            description: "The tracks of one sequence and the clips on them, each clip with \
                its id, its name, where it starts and ends in frames and in timecode, the \
                media it reads and the clips linked to it. Give from and to to read a long \
                timeline in pieces; a clip lying across the edge is reported by both."
                .into(),
            input_schema: schema_of::<queries::TimelineArgs>(),
            annotations: Annotations::READ,
            surfaces: vec![Surface::InApp, Surface::External],
            in_default_surface: true,
        },
        Tool {
            name: "query.at".into(),
            area: Area::Query,
            title: "What is playing".into(),
            description: "Every clip playing at one point, picture and sound, in the order \
                they are drawn: lowest track first, so the last one listed is in front. \
                Disabled clips and disabled tracks are left out, as they are in the render."
                .into(),
            input_schema: schema_of::<queries::AtArgs>(),
            annotations: Annotations::READ,
            surfaces: vec![Surface::InApp, Surface::External],
            in_default_surface: true,
        },
        Tool {
            name: "query.media".into(),
            area: Area::Query,
            title: "Read the bin".into(),
            description: "Everything in the bin, with its size, length and format, whether \
                its file is still where it was, and how many clips read it. That last is \
                what to check before removing one."
                .into(),
            input_schema: schema::nothing(),
            annotations: Annotations::READ,
            surfaces: vec![Surface::InApp, Surface::External],
            in_default_surface: false,
        },
        Tool {
            name: "query.clips".into(),
            area: Area::Query,
            title: "Find clips".into(),
            description: "The clips matching every condition given, and all of them when \
                none is. One search rather than one tool per question: which clips use this \
                media, what is on V2 between these timecodes, which are disabled."
                .into(),
            input_schema: schema_of::<queries::FindArgs>(),
            annotations: Annotations::READ,
            surfaces: vec![Surface::InApp, Surface::External],
            in_default_surface: false,
        },
    ]
}

/// Run a read-only tool.
///
/// Edits are not here yet; when they arrive they need a session that can be written to
/// and a revision to check against, which is a different signature.
pub fn ask(session: &dyn Session, name: &str, arguments: Value) -> Result<Value, ToolError> {
    let revision = session.revision();
    let Some(tool) = find(name) else {
        return Err(ToolError::unknown_tool(name, revision));
    };
    // An absent argument object means "no arguments", which is what a tool taking none
    // is always called with.
    let arguments = if arguments.is_null() {
        Value::Object(Default::default())
    } else {
        arguments
    };
    match tool.name.as_str() {
        "query.overview" => queries::overview(session),
        "query.timeline" => queries::timeline(session, parse(arguments, revision)?),
        "query.at" => queries::at(session, parse(arguments, revision)?),
        "query.media" => queries::media(session, &[]),
        "query.clips" => queries::clips(session, parse(arguments, revision)?),
        _ => Err(ToolError::unknown_tool(name, revision)),
    }
}

/// Arguments into the shape a tool wants, with a refusal the caller can read rather
/// than serde's own wording alone.
fn parse<T: serde::de::DeserializeOwned>(value: Value, revision: u64) -> Result<T, ToolError> {
    serde_json::from_value(value).map_err(|e| ToolError::bad_arguments(e.to_string(), revision))
}

/// Bring media into the bin.
///
/// The one command that cannot be a tool as it stands: `ImportMedia` takes assets
/// already inspected, down to their size and modification time, and a caller cannot
/// know those. So the host reads the files and this applies the ordinary command with
/// what it found, which keeps adding to the bin on the same validated, undoable path as
/// every other edit.
pub fn import(
    session: &mut dyn Session,
    host: &dyn Host,
    paths: &[String],
    expected_revision: Option<u64>,
) -> Result<Import, ToolError> {
    let revision = session.revision();
    if let Some(expected) = expected_revision {
        if expected != revision {
            return Err(ToolError::stale(expected, revision));
        }
    }
    if paths.is_empty() {
        return Err(ToolError::bad_arguments("no paths given", revision));
    }
    let (assets, failed) = host.probe(paths)?;
    if assets.is_empty() {
        return Ok(Import {
            outcome: None,
            failed: failed.into_iter().map(|f| (f.path, f.reason)).collect(),
        });
    }
    let before = session.project().clone();
    let command = studio_core::edit::EditCommand::ImportMedia { assets };
    let report = session
        .apply(&command)
        .map_err(|e| ToolError::from_edit(e, revision))?;
    Ok(Import {
        outcome: Some(Outcome {
            label: command.label().to_string(),
            revision: session.revision(),
            delta: delta::delta(&before, session.project()),
            report,
        }),
        failed: failed.into_iter().map(|f| (f.path, f.reason)).collect(),
    })
}

/// What an import did, and what it could not read.
///
/// A file that could not be read is not an error for the whole call: importing six
/// files of which one is a text document should add five and say so.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Import {
    /// Absent when nothing could be read, so nothing was applied.
    pub outcome: Option<Outcome>,
    pub failed: Vec<(String, String)>,
}

/// Do something that changes the document.
///
/// `expected_revision` is how a caller says what it was looking at. The document
/// refuses rather than applying an edit aimed at a state it has moved on from, which is
/// the difference between a caller that is wrong and a document that is.
pub fn act(
    session: &mut dyn Session,
    name: &str,
    arguments: Value,
    expected_revision: Option<u64>,
) -> Result<Outcome, ToolError> {
    let revision = session.revision();
    if let Some(expected) = expected_revision {
        if expected != revision {
            return Err(ToolError::stale(expected, revision));
        }
    }
    let command = commands::to_command(session, name, arguments)?;
    let label = command.label();
    // Kept so the edit can be compared against it afterwards. A command reports what it
    // created; only the two states together say what it removed and changed.
    let before = session.project().clone();
    let report = session
        .apply(&command)
        .map_err(|e| ToolError::from_edit(e, revision))?;
    Ok(Outcome {
        label: label.to_string(),
        revision: session.revision(),
        delta: delta::delta(&before, session.project()),
        report,
    })
}

/// What an edit did.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    /// How the edit reads in the undo history.
    pub label: String,
    /// Where the document is now, to be handed back on the next call.
    pub revision: u64,
    /// What the edit did, including what it did that nobody asked for.
    pub delta: Delta,
    /// What the command itself reports having created.
    pub report: studio_core::edit::EditReport,
}

/// Every command the document has, and the schema for its arguments.
///
/// Not tools yet: this is the raw material the edit tools are built from, kept public so
/// the test that every command reaches a tool can read it.
pub fn command_schemas() -> BTreeMap<String, Value> {
    schema::per_variant(&schema_of::<studio_core::edit::EditCommand>())
}
