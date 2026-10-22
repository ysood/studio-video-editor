//! The editing session: current project, undo and redo stacks, and dirty tracking.
//!
//! Undo stores whole project snapshots. Projects are small (kilobytes), so this is simpler
//! and more robust than inverse commands, and it makes every command undoable for free.

use crate::edit::{apply, EditCommand, EditError, EditReport};
use crate::model::Project;
use serde::Serialize;
use ts_rs::TS;

/// Undo depth. At about 50 KB per snapshot this bounds history near 25 MB.
pub const MAX_UNDO: usize = 500;

#[derive(Debug, Clone)]
struct Entry {
    label: &'static str,
    project: Project,
    revision: u64,
}

#[derive(Debug, Clone)]
pub struct Editor {
    project: Project,
    revision: u64,
    next_revision: u64,
    saved_revision: Option<u64>,
    undo: Vec<Entry>,
    redo: Vec<Entry>,
}

/// What the UI needs to render the session.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryState {
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_label: Option<String>,
    pub redo_label: Option<String>,
    pub dirty: bool,
    /// Which state the document is in. See [`Editor::revision`].
    ///
    /// A number rather than a bigint on the TypeScript side: this is compared and sent
    /// back across the boundary, and `JSON.stringify` refuses a bigint. One count per
    /// edit will not reach the range where that conversion stops being exact.
    #[ts(type = "number")]
    pub revision: u64,
}

impl Editor {
    /// A session for a project that has never been saved, so it starts dirty.
    pub fn new(project: Project) -> Self {
        Self {
            project,
            revision: 0,
            next_revision: 1,
            saved_revision: None,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// A session for a project just loaded from disk, so it starts clean.
    pub fn opened(project: Project) -> Self {
        let mut editor = Self::new(project);
        editor.saved_revision = Some(editor.revision);
        editor
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    /// Which state the document is in.
    ///
    /// A number is taken by each command that changes something, so a caller that holds
    /// one can tell whether the document is still as it last saw it. That is what lets a
    /// second writer, an agent or another window, refuse an edit aimed at a document
    /// that has since moved rather than apply it to the wrong one.
    ///
    /// It identifies a state, not a point in time, and it does not only count upwards:
    /// undo restores the number the state had before, which is deliberate and is the
    /// same property `is_dirty` relies on to call the document clean again when you undo
    /// back to what was saved. A command that changes nothing leaves it alone.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Apply a command. A command that changes nothing is not recorded in history.
    pub fn apply(&mut self, command: &EditCommand) -> Result<EditReport, EditError> {
        let (next, report) = apply(&self.project, command)?;
        if next == self.project {
            return Ok(report);
        }
        let previous = std::mem::replace(&mut self.project, next);
        self.undo.push(Entry {
            label: command.label(),
            project: previous,
            revision: self.revision,
        });
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.revision = self.next_revision;
        self.next_revision += 1;
        Ok(report)
    }

    pub fn undo(&mut self) -> bool {
        self.step(true)
    }

    pub fn redo(&mut self) -> bool {
        self.step(false)
    }

    fn step(&mut self, backwards: bool) -> bool {
        let (from, to) = if backwards {
            (&mut self.undo, &mut self.redo)
        } else {
            (&mut self.redo, &mut self.undo)
        };
        let Some(entry) = from.pop() else {
            return false;
        };
        let current = Entry {
            label: entry.label,
            project: std::mem::replace(&mut self.project, entry.project),
            revision: self.revision,
        };
        self.revision = entry.revision;
        to.push(current);
        true
    }

    /// Record that the current state is what is on disk.
    pub fn mark_saved(&mut self) {
        self.saved_revision = Some(self.revision);
    }

    pub fn is_dirty(&self) -> bool {
        self.saved_revision != Some(self.revision)
    }

    pub fn history_state(&self) -> HistoryState {
        HistoryState {
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
            undo_label: self.undo.last().map(|e| e.label.to_string()),
            redo_label: self.redo.last().map(|e| e.label.to_string()),
            dirty: self.is_dirty(),
            revision: self.revision,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{media_with_av, project_with};

    fn rename(name: &str) -> EditCommand {
        EditCommand::RenameProject { name: name.into() }
    }

    #[test]
    fn undo_and_redo_restore_exact_states() {
        let mut editor = Editor::opened(project_with(vec![media_with_av(5)]));
        editor.apply(&rename("A")).unwrap();
        editor.apply(&rename("B")).unwrap();
        assert!(editor.undo());
        assert_eq!(editor.project().name, "A");
        assert!(editor.undo());
        assert_eq!(editor.project().name, "Test");
        assert!(!editor.undo());
        assert!(editor.redo());
        assert_eq!(editor.project().name, "A");
    }

    /// The three properties a second writer leans on when it holds a revision and asks
    /// whether the document is still where it left it.
    #[test]
    fn revision_identifies_the_state() {
        let mut editor = Editor::opened(project_with(vec![]));
        let start = editor.revision();

        // A command that changes something moves it.
        editor.apply(&rename("A")).unwrap();
        let after_a = editor.revision();
        assert_ne!(after_a, start);

        // A command that changes nothing does not, so holding a revision across one is
        // safe: renaming to the name it already has is not a reason to refuse an edit.
        editor.apply(&rename("A")).unwrap();
        assert_eq!(editor.revision(), after_a, "no-op left the revision alone");

        // Undo goes back to the number that state had, rather than inventing a new one.
        // Without this, undoing would look like a change to anyone holding `start`.
        assert!(editor.undo());
        assert_eq!(editor.revision(), start);
        assert!(editor.redo());
        assert_eq!(editor.revision(), after_a);
    }

    #[test]
    fn each_change_gets_its_own_revision() {
        let mut editor = Editor::opened(project_with(vec![]));
        let mut seen = vec![editor.revision()];
        for name in ["A", "B", "C"] {
            editor.apply(&rename(name)).unwrap();
            seen.push(editor.revision());
        }
        let mut unique = seen.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(
            unique.len(),
            seen.len(),
            "a revision was handed out twice: {seen:?}"
        );
    }

    #[test]
    fn history_state_carries_the_revision() {
        let mut editor = Editor::opened(project_with(vec![]));
        editor.apply(&rename("A")).unwrap();
        assert_eq!(editor.history_state().revision, editor.revision());
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut editor = Editor::opened(project_with(vec![]));
        editor.apply(&rename("A")).unwrap();
        editor.undo();
        editor.apply(&rename("C")).unwrap();
        assert!(!editor.history_state().can_redo);
    }

    #[test]
    fn dirty_tracks_saved_revision_through_undo() {
        let mut editor = Editor::opened(project_with(vec![]));
        assert!(!editor.is_dirty());
        editor.apply(&rename("A")).unwrap();
        assert!(editor.is_dirty());
        editor.undo();
        assert!(!editor.is_dirty(), "back at the saved state");
        editor.redo();
        editor.mark_saved();
        editor.undo();
        assert!(editor.is_dirty());
    }

    #[test]
    fn rejected_and_no_op_commands_leave_no_history() {
        let mut editor = Editor::opened(project_with(vec![]));
        assert!(editor.apply(&rename("   ")).is_err());
        editor.apply(&rename("Test")).unwrap();
        assert!(!editor.history_state().can_undo);
    }

    #[test]
    fn history_labels_name_the_command() {
        let mut editor = Editor::new(project_with(vec![]));
        editor.apply(&rename("A")).unwrap();
        assert_eq!(
            editor.history_state().undo_label.as_deref(),
            Some("Rename project")
        );
    }
}
