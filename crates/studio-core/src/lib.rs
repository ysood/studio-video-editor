//! Studio's core: the project document and everything that reads or changes it.
//!
//! The Tauri app, tests and future automation all go through this crate, so the
//! document has one definition and one set of rules.

pub mod edit;
pub mod history;
pub mod ids;
pub mod model;
pub mod time;
pub mod validate;

#[cfg(test)]
pub(crate) mod testing;

pub use edit::{apply, ClipEdge, EditCommand, EditError, EditReport};
pub use history::{Editor, HistoryState};
pub use ids::*;
pub use model::*;
pub use time::{timecode, Frame, Micros, Rational};
pub use validate::ValidationError;
