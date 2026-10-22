//! IPC commands. Each one validates its input, calls into `studio-core`, and returns a
//! fresh [`Snapshot`]. Long work (probing, rendering, packaging) runs off the main thread.

use crate::error::{CommandError, CommandResult, ErrorCode};
use crate::menu;
use crate::state::{AppState, Job, Session, Snapshot};
use crate::tools::{self, ToolsStatus};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use studio_core::export::render::{self, RenderSettings};
use studio_core::export::{fcp7, otio, package};
use studio_core::{
    persist, probe, EditCommand, EditReport, Editor, MediaAsset, Project, SequenceFormat,
    SequenceId,
};
use tauri::{AppHandle, Emitter, Manager, State};
use ts_rs::TS;

/// Imports above this count are probed in batches of this many parallel ffprobe runs.
const PROBE_PARALLELISM: usize = 4;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EditResponse {
    pub snapshot: Snapshot,
    pub report: EditReport,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportResponse {
    pub snapshot: Snapshot,
    pub report: EditReport,
    pub failed: Vec<ImportFailure>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportFailure {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum InterchangeFormat {
    Otio,
    Fcp7Xml,
}

/// Emitted as `job://progress` while a render or package job runs.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JobProgress {
    pub kind: JobKind,
    /// 0.0 to 1.0.
    pub fraction: f64,
}

/// Emitted as `job://finished` when a job ends for any reason.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JobFinished {
    pub kind: JobKind,
    pub ok: bool,
    pub cancelled: bool,
    pub output: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum JobKind {
    Render,
    Package,
}

pub const EVENT_PROGRESS: &str = "job://progress";
pub const EVENT_FINISHED: &str = "job://finished";
pub const EVENT_CHANGED: &str = "document://changed";

/// Who moved the document.
///
/// The window needs this to decide what to do about it. Its own edit it already knows
/// about; one made by something else is a reason to look again, and a reason to say so
/// rather than have the timeline change under the person watching it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Source {
    /// The window, by clicking or by the menu.
    App,
    /// Something driving the document through the tool catalogue.
    Agent,
}

/// That the document moved, and to where.
///
/// Deliberately without the document itself. Events go to every webview, the reply to
/// whoever asked already carries the new state, and a document of any size on every
/// edit would be a great deal of copying for something most listeners will ignore.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DocumentChanged {
    #[ts(type = "number")]
    pub revision: u64,
    /// How the change reads in the undo history, when there is one.
    pub label: Option<String>,
    pub source: Source,
}

/// Paths from the UI must be absolute; relative paths would resolve against whatever the
/// process working directory happens to be.
fn absolute(path: &str) -> CommandResult<PathBuf> {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(CommandError::new(
            ErrorCode::BadPath,
            "path must be absolute",
        ))
    }
}

/// Let the webview's media elements read exactly the files the project uses.
pub(crate) fn allow_media(app: &AppHandle, project: &Project) {
    let scope = app.asset_protocol_scope();
    for media in &project.media {
        // A missing file cannot be allowed yet; it shows as offline in the UI.
        let _ = scope.allow_file(&media.path);
    }
}

/// Snapshot the session, release the lock, then bring the native menu up to date. The
/// order matters: menu updates run on the main thread, which may be waiting for the lock.
fn reply(app: &AppHandle, slot: MutexGuard<'_, Option<Session>>) -> CommandResult<Snapshot> {
    reply_as(app, slot, Source::App)
}

/// As `reply`, saying who moved the document.
///
/// Every command that changes anything ends here, which is why the change event is
/// emitted here too: one place, rather than one call per command and one forgotten.
pub(crate) fn reply_as(
    app: &AppHandle,
    slot: MutexGuard<'_, Option<Session>>,
    source: Source,
) -> CommandResult<Snapshot> {
    let snapshot = slot.as_ref().ok_or_else(no_project)?.snapshot();
    drop(slot);
    menu::refresh(app, Some(&snapshot));
    let _ = app.emit(
        EVENT_CHANGED,
        DocumentChanged {
            revision: snapshot.history.revision,
            label: snapshot.history.undo_label.clone(),
            source,
        },
    );
    Ok(snapshot)
}

/// Every command below this point needs a project to act on, and says so rather than
/// inventing one.
fn no_project() -> CommandError {
    CommandError::new(ErrorCode::NoProject, "no project is open")
}

/// Whether the native menu bar replaces the in-window File menu.
#[tauri::command]
pub fn native_menu_active(app: AppHandle) -> bool {
    menu::is_installed(&app)
}

#[tauri::command]
pub fn session_snapshot(state: State<'_, AppState>) -> Option<Snapshot> {
    state.session().as_ref().map(Session::snapshot)
}

#[tauri::command]
pub fn tools_status() -> ToolsStatus {
    tools::status()
}

#[tauri::command]
pub fn project_new(
    name: String,
    format: SequenceFormat,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<Snapshot> {
    let project = Project::new(name.trim(), format);
    project.validate().map_err(studio_core::EditError::from)?;
    let mut slot = state.session();
    *slot = Some(Session {
        editor: Editor::new(project),
        path: None,
    });
    reply(&app, slot)
}

#[tauri::command]
pub fn project_open(
    path: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<Snapshot> {
    let path = absolute(&path)?;
    let project = persist::load(&path)?;
    allow_media(&app, &project);
    crate::recents::record(&app, &path, &project.name);
    let mut slot = state.session();
    *slot = Some(Session {
        editor: Editor::opened(project),
        path: Some(path),
    });
    reply(&app, slot)
}

/// Save to `path`, or to the current path when `path` is `None`.
#[tauri::command]
pub fn project_save(
    path: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<Snapshot> {
    let mut slot = state.session();
    let session = slot.as_mut().ok_or_else(no_project)?;
    let target = match path {
        Some(p) => absolute(&p)?,
        None => session
            .path
            .clone()
            .ok_or_else(|| CommandError::new(ErrorCode::NeedsPath, "choose where to save"))?,
    };
    persist::save(session.editor.project(), &target)?;
    session.editor.mark_saved();
    // Saving is also how a project first gets a home, and how it moves, so the list is
    // kept up to date here as well as on open.
    crate::recents::record(&app, &target, &session.editor.project().name);
    session.path = Some(target);
    reply(&app, slot)
}

/// Put the project away and go back to having none open.
///
/// Unsaved work is the UI's business: it asks before calling this, the way it already
/// does before opening another project.
#[tauri::command]
pub fn project_close(app: AppHandle, state: State<'_, AppState>) {
    *state.session() = None;
    menu::refresh(&app, None);
}

/// The projects this machine has opened, newest first (see `recents`).
#[tauri::command]
pub fn recents_list(app: AppHandle) -> Vec<crate::recents::Recent> {
    crate::recents::list(&app)
}

/// Drop one project from the list. The file itself is untouched.
#[tauri::command]
pub fn recents_forget(path: String, app: AppHandle) {
    crate::recents::forget(&app, &path);
}

#[tauri::command]
pub fn edit(
    command: EditCommand,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<EditResponse> {
    let mut slot = state.session();
    let report = slot
        .as_mut()
        .ok_or_else(no_project)?
        .editor
        .apply(&command)?;
    Ok(EditResponse {
        snapshot: reply(&app, slot)?,
        report,
    })
}

#[tauri::command]
pub fn undo(app: AppHandle, state: State<'_, AppState>) -> CommandResult<Snapshot> {
    let mut slot = state.session();
    slot.as_mut().ok_or_else(no_project)?.editor.undo();
    reply(&app, slot)
}

#[tauri::command]
pub fn redo(app: AppHandle, state: State<'_, AppState>) -> CommandResult<Snapshot> {
    let mut slot = state.session();
    slot.as_mut().ok_or_else(no_project)?.editor.redo();
    reply(&app, slot)
}

/// Probe files with ffprobe and add the usable ones to the bin. Files that fail are
/// reported individually so one bad file does not block the rest.
#[tauri::command]
pub async fn media_import(
    paths: Vec<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<ImportResponse> {
    let ffprobe = state.ffprobe().ok_or_else(|| {
        CommandError::new(
            ErrorCode::Tool,
            "ffprobe was not found. Install FFmpeg (brew install ffmpeg).",
        )
    })?;

    let mut assets = Vec::new();
    let mut failed = Vec::new();
    for batch in paths.chunks(PROBE_PARALLELISM) {
        let handles: Vec<_> = batch
            .iter()
            .cloned()
            .map(|path| {
                let ffprobe = ffprobe.clone();
                thread::spawn(move || (path.clone(), probe_file(&ffprobe, &path)))
            })
            .collect();
        for handle in handles {
            match handle.join() {
                Ok((_, Ok(asset))) => assets.push(asset),
                Ok((path, Err(e))) => failed.push(ImportFailure {
                    path,
                    reason: e.message,
                }),
                Err(_) => failed.push(ImportFailure {
                    path: String::new(),
                    reason: "probe thread panicked".into(),
                }),
            }
        }
    }

    let mut slot = state.session();
    let session = slot.as_mut().ok_or_else(no_project)?;
    let report = if assets.is_empty() {
        EditReport::default()
    } else {
        session.editor.apply(&EditCommand::ImportMedia { assets })?
    };
    allow_media(&app, session.editor.project());
    Ok(ImportResponse {
        snapshot: reply(&app, slot)?,
        report,
        failed,
    })
}

pub(crate) fn probe_file(ffprobe: &Path, path: &str) -> CommandResult<MediaAsset> {
    let path = absolute(path)?;
    if !path.is_file() {
        return Err(CommandError::new(ErrorCode::BadPath, "not a file"));
    }
    let output = Command::new(ffprobe)
        .args(probe::FFPROBE_ARGS)
        .arg(format!("file:{}", path.display()))
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Err(CommandError::new(
            ErrorCode::Probe,
            "not a media file FFmpeg can read",
        ));
    }
    let json = String::from_utf8_lossy(&output.stdout);
    Ok(probe::parse(&json, &path, probe::fingerprint(&path)?)?)
}

#[tauri::command]
pub fn export_interchange(
    sequence_id: SequenceId,
    format: InterchangeFormat,
    path: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let path = absolute(&path)?;
    let text = {
        let slot = state.session();
        let project = slot.as_ref().ok_or_else(no_project)?.editor.project();
        let sequence = project.sequence(sequence_id).ok_or_else(|| {
            CommandError::new(ErrorCode::NotFound, "that sequence is not in the project")
        })?;
        match format {
            InterchangeFormat::Otio => otio::export(project, sequence),
            InterchangeFormat::Fcp7Xml => fcp7::export(project, sequence),
        }
    };
    std::fs::write(path, text)?;
    Ok(())
}

#[tauri::command]
pub fn export_package(
    sequence_id: SequenceId,
    folder: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let root = absolute(&folder)?;
    let mut job = state.job.lock().unwrap_or_else(|e| e.into_inner());
    if job.is_some() {
        return Err(CommandError::new(
            ErrorCode::Busy,
            "another export is running",
        ));
    }
    let project = state
        .session()
        .as_ref()
        .ok_or_else(no_project)?
        .editor
        .project()
        .clone();
    let sequence = project.sequence(sequence_id).ok_or_else(|| {
        CommandError::new(ErrorCode::NotFound, "that sequence is not in the project")
    })?;
    let plan = package::plan(&project, sequence, &root);
    let total = plan.total_bytes().max(1);
    *job = Some(Job {
        child: Arc::new(Mutex::new(None)),
    });
    drop(job);

    thread::spawn(move || {
        let progress_app = app.clone();
        let result = package::write(&project, &plan, |bytes| {
            let _ = progress_app.emit(
                EVENT_PROGRESS,
                JobProgress {
                    kind: JobKind::Package,
                    fraction: bytes as f64 / total as f64,
                },
            );
        });
        finish(
            &app,
            JobKind::Package,
            root.display().to_string(),
            result.map_err(|e| e.to_string()),
            false,
        );
    });
    Ok(())
}

#[tauri::command]
pub fn render_start(
    sequence_id: SequenceId,
    path: String,
    settings: RenderSettings,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    let output = absolute(&path)?;
    let ffmpeg = state.ffmpeg().ok_or_else(|| {
        CommandError::new(
            ErrorCode::Tool,
            "ffmpeg was not found. Install FFmpeg (brew install ffmpeg).",
        )
    })?;
    let mut job = state.job.lock().unwrap_or_else(|e| e.into_inner());
    if job.is_some() {
        return Err(CommandError::new(
            ErrorCode::Busy,
            "another export is running",
        ));
    }
    let plan = {
        let slot = state.session();
        let project = slot.as_ref().ok_or_else(no_project)?.editor.project();
        let sequence = project.sequence(sequence_id).ok_or_else(|| {
            CommandError::new(ErrorCode::NotFound, "that sequence is not in the project")
        })?;
        render::plan(project, sequence, settings, &output)?
    };

    let mut child = Command::new(ffmpeg)
        .args(&plan.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");
    let shared = Arc::new(Mutex::new(Some(child)));
    *job = Some(Job {
        child: shared.clone(),
    });
    drop(job);

    // Drain stderr on its own thread so a chatty FFmpeg never blocks on a full pipe.
    let stderr_reader = thread::spawn(move || {
        let mut text = String::new();
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if text.len() < 16 * 1024 {
                text.push_str(&line);
                text.push('\n');
            }
        }
        text
    });

    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(us) = render::parse_progress(&line) {
                let fraction = (us as f64 / plan.duration_us.max(1) as f64).clamp(0.0, 1.0);
                let _ = app.emit(
                    EVENT_PROGRESS,
                    JobProgress {
                        kind: JobKind::Render,
                        fraction,
                    },
                );
            }
        }
        let status = shared
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .map(|mut c| c.wait());
        let errors = stderr_reader.join().unwrap_or_default();
        let (result, cancelled) = match status {
            Some(Ok(s)) if s.success() => (Ok(()), false),
            Some(Ok(_)) => (Err(last_line(&errors)), false),
            Some(Err(e)) => (Err(e.to_string()), false),
            None => (Err("cancelled".to_string()), true),
        };
        if result.is_err() {
            let _ = std::fs::remove_file(&output);
        }
        finish(
            &app,
            JobKind::Render,
            output.display().to_string(),
            result,
            cancelled,
        );
    });
    Ok(())
}

#[tauri::command]
pub fn render_cancel(state: State<'_, AppState>) {
    let job = state.job.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(job) = job.as_ref() {
        if let Some(mut child) = job.child.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn finish(
    app: &AppHandle,
    kind: JobKind,
    output: String,
    result: Result<(), String>,
    cancelled: bool,
) {
    if let Some(state) = app.try_state::<AppState>() {
        *state.job.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
    let _ = app.emit(
        EVENT_FINISHED,
        JobFinished {
            kind,
            ok: result.is_ok(),
            cancelled,
            output,
            message: result.err(),
        },
    );
}

fn last_line(text: &str) -> String {
    text.lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("ffmpeg failed")
        .to_string()
}

/// Position the native preview surface over the viewer's matte (ADR-0003). The rect
/// arrives in CSS pixels with a top left origin, straight from `getBoundingClientRect`.
#[tauri::command]
pub fn viewer_set_rect(window: tauri::WebviewWindow, x: f64, y: f64, width: f64, height: f64) {
    #[cfg(target_os = "macos")]
    let _ = window.with_webview(move |webview| {
        crate::viewer::set_rect(webview.ns_window(), x, y, width, height);
    });
    #[cfg(not(target_os = "macos"))]
    let _ = (window, x, y, width, height);
}

/// Rebuild the preview composition from the sequence that is open. Called after any edit
/// that changes what plays, and when the open sequence changes; the composition is
/// derived, never a source of truth.
///
/// A sequence that has since been deleted leaves the picture alone rather than clearing
/// it, because the UI resyncs with the sequence it falls back to.
#[tauri::command]
pub fn viewer_sync(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    sequence_id: SequenceId,
    frame: i64,
) {
    #[cfg(target_os = "macos")]
    {
        let Some((segments, canvas, at_us)) = ({
            let slot = state.session();
            slot.as_ref().and_then(|session| {
                let project = session.editor.project();
                project.sequence(sequence_id).map(|sequence| {
                    (
                        crate::viewer::segments(project, sequence),
                        crate::viewer::Canvas::of(sequence),
                        sequence.format.frame_rate.frames_to_micros(frame),
                    )
                })
            })
        }) else {
            return;
        };
        let app = window.app_handle().clone();
        let pointer_app = app.clone();
        let _ = window.with_webview(move |webview| {
            let ns_window = webview.ns_window();
            crate::viewer::set_segments(ns_window, &segments, canvas, at_us);
            // The first sync installs the player clock. AVFoundation owns playback time;
            // the UI follows it rather than running a clock of its own.
            crate::viewer::observe_time(ns_window, move |micros| {
                let _ = app.emit("viewer://time", micros);
            });
            // And the mouse, for the same reason: the surface is above the webview, so
            // nothing over the picture is heard unless it is reported (ADR-0006).
            crate::viewer::watch_pointer(ns_window, move |pointer| {
                let _ = pointer_app.emit(
                    "viewer://pointer",
                    ViewerPointer {
                        phase: pointer.phase.name().to_string(),
                        x: pointer.x,
                        y: pointer.y,
                        shift: pointer.shift,
                    },
                );
            });
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (window, state, sequence_id, frame);
}

/// Where the viewer should draw the handles, in the surface's own coordinates with y
/// measured from the bottom. The web layer converts, because it is the side that knows
/// the stage's size (ADR-0006).
#[derive(Debug, Clone, Copy, serde::Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ViewerOverlay {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// `a`, `b`, `c`, `d`, `tx`, `ty` of the clip's transform.
    pub transform: [f64; 6],
}

/// The handles' box, or `None` to take them away. Called whenever the selection or the
/// transform changes, and while a drag previews where the clip is going.
#[tauri::command]
pub fn viewer_set_overlay(window: tauri::WebviewWindow, overlay: Option<ViewerOverlay>) {
    #[cfg(target_os = "macos")]
    let _ = window.with_webview(move |webview| {
        crate::viewer::set_overlay(
            webview.ns_window(),
            overlay.map(|o| crate::viewer::Overlay {
                x: o.x,
                y: o.y,
                width: o.width,
                height: o.height,
                transform: o.transform,
            }),
        );
    });
    #[cfg(not(target_os = "macos"))]
    let _ = (window, overlay);
}

/// What the mouse did over the picture, for the web layer to interpret.
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ViewerPointer {
    /// `down`, `moved` or `up`.
    pub phase: String,
    pub x: f64,
    pub y: f64,
    pub shift: bool,
}

/// Hide or show the native preview surface. The web layer asks for this when it draws
/// something over the viewer, because the surface would otherwise cover it (ADR-0003).
#[tauri::command]
pub fn viewer_set_hidden(window: tauri::WebviewWindow, hidden: bool) {
    #[cfg(target_os = "macos")]
    let _ = window.with_webview(move |webview| {
        crate::viewer::set_hidden(webview.ns_window(), hidden);
    });
    #[cfg(not(target_os = "macos"))]
    let _ = (window, hidden);
}

#[tauri::command]
pub fn viewer_play(window: tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    let _ = window.with_webview(|webview| crate::viewer::play(webview.ns_window()));
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

#[tauri::command]
pub fn viewer_pause(window: tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    let _ = window.with_webview(|webview| crate::viewer::pause(webview.ns_window()));
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

/// Seek the preview to a frame of the sequence that is open. The frame is converted here
/// so the surface never needs to know the sequence rate.
#[tauri::command]
pub fn viewer_seek(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    sequence_id: SequenceId,
    frame: i64,
) {
    #[cfg(target_os = "macos")]
    {
        let Some(micros) = ({
            let slot = state.session();
            slot.as_ref().and_then(|session| {
                session
                    .editor
                    .project()
                    .sequence(sequence_id)
                    .map(|s| s.format.frame_rate.frames_to_micros(frame))
            })
        }) else {
            return;
        };
        let _ = window.with_webview(move |webview| {
            crate::viewer::seek(webview.ns_window(), micros);
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (window, state, sequence_id, frame);
}
