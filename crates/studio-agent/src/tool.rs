//! What a tool is, before any question of how it is carried.
//!
//! The shapes here are the ones the Model Context Protocol asks for, because that is
//! the format callers will want and inventing a second one would mean translating
//! twice. They are not tied to it: nothing in this crate speaks a wire protocol, and an
//! adapter decides how a tool and its result are rendered.

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

/// What a tool is about. The name of every tool begins with its area, so a caller
/// reading a list sees them grouped, and so a whole area can be turned on or off at
/// once without naming each tool in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Area {
    /// The project as a whole.
    Project,
    Sequence,
    Track,
    /// Putting clips on the timeline and moving them about.
    Timeline,
    /// Changing a clip that is already placed.
    Clip,
    /// The bin.
    Media,
    /// Reading the document. Everything here is read only.
    Query,
    /// Undo and redo.
    History,
    Export,
}

impl Area {
    pub fn as_str(self) -> &'static str {
        match self {
            Area::Project => "project",
            Area::Sequence => "sequence",
            Area::Track => "track",
            Area::Timeline => "timeline",
            Area::Clip => "clip",
            Area::Media => "media",
            Area::Query => "query",
            Area::History => "history",
            Area::Export => "export",
        }
    }
}

/// Where a tool may be offered.
///
/// Not every tool is safe everywhere. A pane inside the app is driving the document its
/// own window is showing, with the person watching, so it may undo. A caller in another
/// process is not: an undo there is a rewind of whatever the person was doing, and
/// nothing in the protocol says whose edit is being undone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Surface {
    /// A pane inside the app, driving the document on screen.
    InApp,
    /// Anything in another process.
    External,
}

/// How a tool behaves, for a caller deciding whether to ask first.
///
/// These are the Model Context Protocol's hints, and its defaults are pessimistic: left
/// unsaid, a tool is taken to be destructive and to reach an open world. Almost nothing
/// here is either, so every tool says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Annotations {
    /// Changes nothing.
    pub read_only: bool,
    /// May remove or overwrite something, rather than only adding.
    ///
    /// Read as "content can be lost": a command that sets a field to a value you can
    /// read back first is not destructive, or every setter would be and the hint would
    /// stop carrying information.
    pub destructive: bool,
    /// Calling it twice with the same arguments leaves the document as one call did.
    pub idempotent: bool,
    /// Reaches something outside the document, such as the filesystem.
    pub open_world: bool,
}

impl Annotations {
    /// A question. Changes nothing, so the rest does not arise.
    pub const READ: Self = Self {
        read_only: true,
        destructive: false,
        idempotent: true,
        open_world: false,
    };

    /// Changes the document, adds rather than removes, and reaches nothing outside it.
    pub const ADDS: Self = Self {
        read_only: false,
        destructive: false,
        idempotent: false,
        open_world: false,
    };

    /// Changes the document in place: nothing is lost and doing it twice changes nothing
    /// the first did not.
    pub const SETS: Self = Self {
        read_only: false,
        destructive: false,
        idempotent: true,
        open_world: false,
    };

    /// Content can be lost.
    pub const REMOVES: Self = Self {
        read_only: false,
        destructive: true,
        idempotent: true,
        open_world: false,
    };

    pub const fn not_idempotent(mut self) -> Self {
        self.idempotent = false;
        self
    }

    pub const fn reaching_outside(mut self) -> Self {
        self.open_world = true;
        self
    }
}

/// One thing a caller can do, and everything it needs to decide whether to.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Tool {
    /// `area.verb_noun`, which is what makes a list of these readable in one glance.
    pub name: String,
    pub area: Area,
    /// A few words for a person reading a list.
    pub title: String,
    /// What it does, what it refuses and what it reaches. For the commands this is the
    /// doc comment above the variant, carried through by the schema.
    pub description: String,
    /// JSON Schema for the arguments.
    #[ts(type = "unknown")]
    pub input_schema: Value,
    pub annotations: Annotations,
    /// Where it may be offered.
    pub surfaces: Vec<Surface>,
    /// Whether it is offered unless someone says otherwise. Most tools are not: a long
    /// list is the usual way a caller gets worse rather than better, because every
    /// entry costs it attention on every turn.
    pub in_default_surface: bool,
}

impl Tool {
    pub fn available_on(&self, surface: Surface) -> bool {
        self.surfaces.contains(&surface)
    }
}
