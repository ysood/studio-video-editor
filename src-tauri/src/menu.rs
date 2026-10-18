//! The native application menu (macOS menu bar). Items only name an action; choosing one
//! emits `menu://action` and the UI runs the same code path as its own buttons and
//! shortcuts. Rust refreshes item state (undo labels, what is enabled) from each snapshot,
//! since it owns the history.
//!
//! On other platforms the UI draws its own File menu in the top bar, because a native menu
//! there is a second bar inside the window. `STUDIO_NATIVE_MENU=1` forces the native menu
//! on any platform for testing.

use crate::state::Snapshot;
use serde::Serialize;
use tauri::menu::{AboutMetadata, MenuBuilder, MenuItem, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use ts_rs::TS;

pub const EVENT_MENU_ACTION: &str = "menu://action";

/// Every action a native menu item can trigger. The serialized name is also the item id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MenuAction {
    NewProject,
    OpenProject,
    CloseProject,
    Save,
    SaveAs,
    ImportMedia,
    Export,
    Undo,
    Redo,
    ResetLayout,
}

impl MenuAction {
    const ALL: [MenuAction; 10] = [
        MenuAction::NewProject,
        MenuAction::OpenProject,
        MenuAction::CloseProject,
        MenuAction::Save,
        MenuAction::SaveAs,
        MenuAction::ImportMedia,
        MenuAction::Export,
        MenuAction::Undo,
        MenuAction::Redo,
        MenuAction::ResetLayout,
    ];

    pub fn id(self) -> &'static str {
        match self {
            MenuAction::NewProject => "newProject",
            MenuAction::OpenProject => "openProject",
            MenuAction::CloseProject => "closeProject",
            MenuAction::Save => "save",
            MenuAction::SaveAs => "saveAs",
            MenuAction::ImportMedia => "importMedia",
            MenuAction::Export => "export",
            MenuAction::Undo => "undo",
            MenuAction::Redo => "redo",
            MenuAction::ResetLayout => "resetLayout",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.id() == id)
    }
}

/// Items whose state follows the document.
pub struct NativeMenu<R: Runtime> {
    undo: MenuItem<R>,
    redo: MenuItem<R>,
    export: MenuItem<R>,
}

/// Whether the native menu should replace the in-window one.
pub fn wanted() -> bool {
    cfg!(target_os = "macos") || std::env::var_os("STUDIO_NATIVE_MENU").is_some_and(|v| v == "1")
}

pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let item = |action: MenuAction, text: &str, accelerator: Option<&str>| {
        let mut builder = MenuItemBuilder::with_id(action.id(), text);
        if let Some(accelerator) = accelerator {
            builder = builder.accelerator(accelerator);
        }
        builder.build(app)
    };

    let undo = item(MenuAction::Undo, "Undo", Some("CmdOrCtrl+Z"))?;
    let redo = item(MenuAction::Redo, "Redo", Some("CmdOrCtrl+Shift+Z"))?;
    let export = item(MenuAction::Export, "Export…", Some("CmdOrCtrl+E"))?;

    let app_menu = SubmenuBuilder::new(app, "Studio")
        .about(Some(AboutMetadata {
            name: Some("Studio".into()),
            ..Default::default()
        }))
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let file = SubmenuBuilder::new(app, "File")
        .item(&item(
            MenuAction::NewProject,
            "New Project",
            Some("CmdOrCtrl+N"),
        )?)
        .item(&item(
            MenuAction::OpenProject,
            "Open…",
            Some("CmdOrCtrl+O"),
        )?)
        .item(&item(
            MenuAction::CloseProject,
            "Close Project",
            Some("CmdOrCtrl+W"),
        )?)
        .separator()
        .item(&item(MenuAction::Save, "Save", Some("CmdOrCtrl+S"))?)
        .item(&item(
            MenuAction::SaveAs,
            "Save As…",
            Some("CmdOrCtrl+Shift+S"),
        )?)
        .separator()
        .item(&item(
            MenuAction::ImportMedia,
            "Import Media…",
            Some("CmdOrCtrl+I"),
        )?)
        .item(&export)
        .build()?;

    // Cut, copy, paste and select all stay native so text fields behave as they do in
    // every other Mac app. Undo and redo are ours; the UI forwards them to a focused text
    // field when there is one.
    let edit = SubmenuBuilder::new(app, "Edit")
        .item(&undo)
        .item(&redo)
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let view = SubmenuBuilder::new(app, "View")
        .item(&item(MenuAction::ResetLayout, "Reset Layout", None)?)
        .separator()
        .fullscreen()
        .build()?;

    let window = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .separator()
        .close_window()
        .build()?;

    let menu = MenuBuilder::new(app)
        .items(&[&app_menu, &file, &edit, &view, &window])
        .build()?;
    app.set_menu(menu)?;
    app.manage(NativeMenu { undo, redo, export });

    app.on_menu_event(|app, event| {
        if let Some(action) = MenuAction::from_id(event.id().as_ref()) {
            let _ = app.emit(EVENT_MENU_ACTION, action);
        }
    });
    Ok(())
}

pub fn is_installed<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<NativeMenu<R>>().is_some()
}

/// Bring item labels and enabled states in line with `snapshot`, or with no project open
/// when it is `None`. Call it without holding the session lock: menu updates run on the
/// main thread, which may be waiting for it.
pub fn refresh<R: Runtime>(app: &AppHandle<R>, snapshot: Option<&Snapshot>) {
    let Some(menu) = app.try_state::<NativeMenu<R>>() else {
        return;
    };
    let label = |verb: &str, step: &Option<String>| match step {
        Some(step) => format!("{verb} {step}"),
        None => verb.to_string(),
    };
    let has_clips = snapshot.is_some_and(|s| {
        s.project
            .first_sequence()
            .tracks
            .iter()
            .any(|t| !t.clips.is_empty())
    });
    // Undo and redo stay enabled even with no history: they are also how a focused text
    // field gets Cmd+Z, and a disabled item would swallow nothing and do nothing.
    // Menu updates only fail when the menu is being torn down at exit.
    // With no project open there is nothing to undo, so the items keep their bare verbs.
    let _ = menu.undo.set_text(label(
        "Undo",
        &snapshot.and_then(|s| s.history.undo_label.clone()),
    ));
    let _ = menu.redo.set_text(label(
        "Redo",
        &snapshot.and_then(|s| s.history.redo_label.clone()),
    ));
    let _ = menu.export.set_enabled(has_clips);
}

#[cfg(test)]
mod tests {
    use super::MenuAction;

    #[test]
    fn ids_round_trip_and_match_the_serialized_names() {
        for action in MenuAction::ALL {
            assert_eq!(MenuAction::from_id(action.id()), Some(action));
            let json = serde_json::to_value(action).unwrap();
            assert_eq!(json, action.id());
        }
    }
}
