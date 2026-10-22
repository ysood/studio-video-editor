//! The two things a tool needs of whoever is holding the document.
//!
//! This is deliberately narrow. Almost every tool is a pure function of the project: it
//! reads the document, resolves what the caller named into ids, and returns a command.
//! Only applying that command needs anything stateful, so that is all this asks for.
//!
//! Keeping it to three methods is what makes the same catalogue work in two places. The
//! shell implements it over the session its window is showing; a process with no window
//! implements it over an [`Editor`] it loaded from a file, and since `Editor` satisfies
//! it already, that second case costs nothing.

use studio_core::edit::{EditCommand, EditError, EditReport};
use studio_core::history::Editor;
use studio_core::model::Project;

/// Whoever is holding the document.
pub trait Session {
    fn project(&self) -> &Project;

    /// Which state the document is in, so a caller can be told that what it was looking
    /// at has moved.
    fn revision(&self) -> u64;

    /// Apply a command. Validation, undo and the refusal of anything that would leave
    /// the document invalid all belong to whoever implements this, which in practice
    /// means they belong to `Editor`.
    fn apply(&mut self, command: &EditCommand) -> Result<EditReport, EditError>;
}

impl Session for Editor {
    fn project(&self) -> &Project {
        Editor::project(self)
    }

    fn revision(&self) -> u64 {
        Editor::revision(self)
    }

    fn apply(&mut self, command: &EditCommand) -> Result<EditReport, EditError> {
        Editor::apply(self, command)
    }
}
