//! Reading the document.
//!
//! Five questions, not one per thing that can be looked up. A caller choosing between
//! twenty near-identical lookups chooses badly, and each one would cost it attention on
//! every turn whether or not it was used.
//!
//! Every argument that names part of the document takes a selector, so a caller can say
//! `V1@00:00:12:00` before it holds any ids at all.

use crate::error::ToolError;
use crate::session::Session;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use studio_core::model::Sequence;
use studio_core::query::{self, ClipFilter};
use studio_core::select;
use studio_core::time::Frame;

/// Something named in text: an id, a name such as `V1`, a timecode, or a frame.
///
/// A frame may be written either way. A caller that has just read `start: 100` out of a
/// timeline will send `100`, not `"100"`, and refusing that would be a trap of our own
/// making: the grammar says a frame number is a way of naming a time, so both forms are
/// the same way.
#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Selector {
    Text(String),
    Frame(i64),
}

impl Selector {
    pub fn as_text(&self) -> std::borrow::Cow<'_, str> {
        match self {
            Selector::Text(text) => std::borrow::Cow::Borrowed(text),
            Selector::Frame(frame) => std::borrow::Cow::Owned(frame.to_string()),
        }
    }
}

/// Which sequence to read, and where in it.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct TimelineArgs {
    /// Id or name. Left out, the project's first sequence answers, and the reply says
    /// which one that was.
    pub sequence: Option<Selector>,
    /// A frame, a timecode, or `start` / `end`. With both ends given, only the clips
    /// with some part between them come back, which is how a long timeline is read in
    /// pieces.
    pub from: Option<Selector>,
    pub to: Option<Selector>,
}

/// A point in one sequence.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AtArgs {
    /// A frame, a timecode, or `start` / `end`.
    pub at: Selector,
    /// Id or name. Left out, the project's first sequence answers.
    #[serde(default)]
    pub sequence: Option<Selector>,
}

/// Which clips to find. Every field left out widens the search.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
pub struct FindArgs {
    pub sequence: Option<Selector>,
    /// Track ids, names such as `V1`, or `video:1`.
    pub tracks: Vec<Selector>,
    /// A media id, to find every clip reading it.
    pub media: Option<String>,
    /// Matched without regard to case.
    pub name_contains: Option<String>,
    pub from: Option<Selector>,
    pub to: Option<Selector>,
    pub enabled: Option<bool>,
}

/// The sequence a caller named, or the first one when it named none.
fn sequence_of<'a>(
    session: &'a dyn Session,
    named: Option<&Selector>,
) -> Result<&'a Sequence, ToolError> {
    let project = session.project();
    match named.map(Selector::as_text) {
        None => Ok(project.first_sequence()),
        Some(name) => {
            let name = name.as_ref();
            let id = select::sequence(project, name)
                .map_err(|e| ToolError::from_select(e, session.revision()))?;
            project.sequence(id).ok_or_else(|| {
                ToolError::from_select(
                    studio_core::select::SelectError::NotFound {
                        what: "sequence",
                        input: name.to_string(),
                    },
                    session.revision(),
                )
            })
        }
    }
}

fn frame_of(session: &dyn Session, sequence: &Sequence, named: &str) -> Result<Frame, ToolError> {
    select::time(sequence, named).map_err(|e| ToolError::from_select(e, session.revision()))
}

fn track_of(
    session: &dyn Session,
    sequence: &Sequence,
    named: &str,
) -> Result<studio_core::ids::TrackId, ToolError> {
    select::track(sequence, named).map_err(|e| ToolError::from_select(e, session.revision()))
}

fn json<T: serde::Serialize>(value: T) -> Result<Value, ToolError> {
    serde_json::to_value(value)
        .map_err(|e| ToolError::new(crate::error::ToolErrorCode::Host, e.to_string(), 0))
}

pub fn overview(session: &dyn Session) -> Result<Value, ToolError> {
    json(query::overview(session.project()))
}

pub fn timeline(session: &dyn Session, args: TimelineArgs) -> Result<Value, ToolError> {
    let sequence = sequence_of(session, args.sequence.as_ref())?;
    let window = match (&args.from, &args.to) {
        (None, None) => None,
        (from, to) => {
            let from = from
                .as_ref()
                .map_or(Ok(0), |f| frame_of(session, sequence, &f.as_text()))?;
            let to = to.as_ref().map_or(Ok(sequence.duration()), |t| {
                frame_of(session, sequence, &t.as_text())
            })?;
            Some((from, to))
        }
    };
    json(query::describe(session.project(), sequence, window))
}

pub fn at(session: &dyn Session, args: AtArgs) -> Result<Value, ToolError> {
    let sequence = sequence_of(session, args.sequence.as_ref())?;
    let frame = frame_of(session, sequence, &args.at.as_text())?;
    json(query::clips_at(session.project(), sequence, frame))
}

/// The bin. `offline` is whatever the caller found missing on disk; a caller that
/// cannot look says nothing is.
pub fn media(
    session: &dyn Session,
    offline: &[studio_core::ids::MediaId],
) -> Result<Value, ToolError> {
    json(query::media(session.project(), offline))
}

pub fn clips(session: &dyn Session, args: FindArgs) -> Result<Value, ToolError> {
    let sequence = sequence_of(session, args.sequence.as_ref())?;
    let mut tracks = Vec::new();
    for name in &args.tracks {
        tracks.push(track_of(session, sequence, &name.as_text())?);
    }
    let filter = ClipFilter {
        sequence: Some(sequence.id),
        tracks,
        media: args
            .media
            .as_deref()
            .and_then(studio_core::ids::MediaId::parse),
        name_contains: args.name_contains.clone(),
        from: args
            .from
            .as_ref()
            .map(|f| frame_of(session, sequence, &f.as_text()))
            .transpose()?,
        to: args
            .to
            .as_ref()
            .map(|t| frame_of(session, sequence, &t.as_text()))
            .transpose()?,
        enabled: args.enabled,
    };
    json(query::find(session.project(), &filter))
}
