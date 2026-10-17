//! Studio desktop shell. A thin adapter: commands validate input, call `studio-core`,
//! spawn FFmpeg where needed, and return snapshots. No editing logic lives here.

mod commands;
mod error;
mod state;
mod tools;

use state::AppState;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::session_snapshot,
            commands::tools_status,
            commands::project_new,
            commands::project_open,
            commands::project_save,
            commands::edit,
            commands::undo,
            commands::redo,
            commands::media_import,
            commands::export_interchange,
            commands::export_package,
            commands::render_start,
            commands::render_cancel,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Studio");
}
