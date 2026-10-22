//! Studio's core: the project document and everything that reads or changes it.
//!
//! The Tauri app, tests and future automation all go through this crate, so the
//! document has one definition and one set of rules.

pub mod edit;
pub mod export;
pub mod history;
pub mod ids;
pub mod model;
pub mod persist;
pub mod probe;
pub mod query;
pub mod select;
pub mod time;
pub mod validate;

// Public under the `testing` feature so the Tauri shell's tests can build the same
// fixtures instead of duplicating them.
#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use edit::{apply, ClipEdge, EditCommand, EditError, EditReport, TrackChoice};
pub use history::{Editor, HistoryState};
pub use ids::*;
pub use model::*;
pub use time::{timecode, Frame, Micros, Rational};
pub use validate::ValidationError;
