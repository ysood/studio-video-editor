//! The tool catalogue, reachable from inside the app.
//!
//! Two commands: what can be done, and do one. Everything they expose comes from
//! `studio-agent`, so the pane in the window and a caller in another process are
//! offered the same list built from the same definitions, and nothing is described
//! twice.
//!
//! An edit made here goes through the same session, the same validation and the same
//! undo stack as one made by clicking, because it is the same command. The only thing
//! this adds is a name to call it by.

use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use studio_agent::host::ImportFailure as AgentFailure;
use studio_agent::{Host, Policy, Surface, Tool, ToolError, ToolErrorCode};
use studio_core::ids::MediaId;
use studio_core::model::MediaAsset;
use tauri::{AppHandle, State};
use ts_rs::TS;

use crate::commands::{allow_media, probe_file, reply_as, Source};
use crate::state::{AppState, Snapshot};

/// What came back, and the document if it changed.
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentReply {
    /// The answer to a question, or what an edit did.
    #[ts(type = "unknown")]
    pub result: Value,
    /// Present only when the document changed, so the window can show the result of an
    /// edit without asking for it again.
    pub snapshot: Option<Snapshot>,
}

/// What this process can do beyond the document.
struct Shell {
    ffprobe: Option<PathBuf>,
    offline: Vec<MediaId>,
}

impl Host for Shell {
    fn probe(&self, paths: &[String]) -> Result<(Vec<MediaAsset>, Vec<AgentFailure>), ToolError> {
        let ffprobe = self.ffprobe.as_ref().ok_or_else(|| {
            ToolError::new(
                ToolErrorCode::Host,
                "ffprobe was not found. Install FFmpeg (brew install ffmpeg).",
                0,
            )
            .with_remedy("install FFmpeg, then try again")
        })?;
        let mut assets = Vec::new();
        let mut failed = Vec::new();
        for path in paths {
            match probe_file(ffprobe, path) {
                Ok(asset) => assets.push(asset),
                Err(e) => failed.push(AgentFailure {
                    path: path.clone(),
                    reason: e.message,
                }),
            }
        }
        Ok((assets, failed))
    }

    fn offline(&self) -> Vec<MediaId> {
        self.offline.clone()
    }
}

fn no_project() -> ToolError {
    ToolError::new(ToolErrorCode::NotAvailable, "no project is open", 0)
        .with_remedy("open or create a project first")
}

/// What can be done from inside the app.
#[tauri::command]
pub fn agent_list_tools() -> Vec<Tool> {
    studio_agent::surface(&Policy::new(Surface::InApp))
}

/// Do one of them.
///
/// `expected_revision` is how a caller says which state it was looking at. Left out, the
/// edit is applied to whatever the document is now, which is right for a caller that
/// has just read it and wrong for one acting on something it read a while ago.
#[tauri::command]
pub fn agent_call_tool(
    name: String,
    arguments: Option<Value>,
    expected_revision: Option<u64>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<AgentReply, ToolError> {
    run(
        &name,
        arguments.unwrap_or(Value::Null),
        expected_revision,
        &app,
        &state,
    )
}

/// Run one tool.
///
/// The command above is a thin wrapper, and the assistant calls this directly. One path
/// on purpose: a tool the model runs has to go through the same validation, the same
/// undo stack and the same change event as one a pane runs, or the window would end up
/// showing a document that had moved without it.
pub fn run(
    name: &str,
    arguments: Value,
    expected_revision: Option<u64>,
    app: &AppHandle,
    state: &AppState,
) -> Result<AgentReply, ToolError> {
    let name = name.to_string();
    let tool = studio_agent::find(&name).ok_or_else(|| ToolError::unknown_tool(&name, 0))?;

    let mut slot = state.session();
    let session = slot.as_mut().ok_or_else(no_project)?;

    // A question changes nothing, so it neither takes the revision check nor produces a
    // snapshot: there is nothing new to show.
    if tool.annotations.read_only {
        let result = if name == "query.media" {
            let offline = session.snapshot().offline_media;
            studio_agent::queries::media(&session.editor, &offline)?
        } else {
            studio_agent::ask(&session.editor, &name, arguments)?
        };
        return Ok(AgentReply {
            result,
            snapshot: None,
        });
    }

    let result = if name == "media.import" {
        let paths: Vec<String> = arguments
            .get("paths")
            .and_then(|p| serde_json::from_value(p.clone()).ok())
            .ok_or_else(|| {
                ToolError::bad_arguments(
                    "paths must be a list of strings",
                    session.editor.revision(),
                )
            })?;
        let host = Shell {
            ffprobe: state.ffprobe(),
            offline: session.snapshot().offline_media,
        };
        let out = studio_agent::import(&mut session.editor, &host, &paths, expected_revision)?;
        // The webview can only read files it has been told about, and media brought in
        // this way has to be readable the same as media brought in by the dialog.
        allow_media(app, session.editor.project());
        serde_json::to_value(out).unwrap_or(Value::Null)
    } else {
        let out = studio_agent::act(&mut session.editor, &name, arguments, expected_revision)?;
        serde_json::to_value(out).unwrap_or(Value::Null)
    };

    // The same path every other edit takes: snapshot, release the lock, then bring the
    // menu up to date and tell the window the document moved.
    let snapshot = reply_as(app, slot, Source::Agent)
        .map_err(|e| ToolError::new(ToolErrorCode::Host, e.message, 0))?;
    Ok(AgentReply {
        result,
        snapshot: Some(snapshot),
    })
}
