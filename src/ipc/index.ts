// Typed wrappers over the Tauri commands in src-tauri/src/commands.rs. Every type comes
// from src/bindings, which is generated from Rust, so a schema change that is not
// reflected here fails `npm run typecheck`.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AgentReply } from "../bindings/AgentReply";
import type { ChatError } from "../bindings/ChatError";
import type { ChatStatus } from "../bindings/ChatStatus";
import type { CommandError } from "../bindings/CommandError";
import type { DocumentChanged } from "../bindings/DocumentChanged";
import type { EditCommand } from "../bindings/EditCommand";
import type { EditResponse } from "../bindings/EditResponse";
import type { ImportResponse } from "../bindings/ImportResponse";
import type { InterchangeFormat } from "../bindings/InterchangeFormat";
import type { JobFinished } from "../bindings/JobFinished";
import type { JobProgress } from "../bindings/JobProgress";
import type { MenuAction } from "../bindings/MenuAction";
import type { Recent } from "../bindings/Recent";
import type { RenderSettings } from "../bindings/RenderSettings";
import type { SequenceFormat } from "../bindings/SequenceFormat";
import type { Snapshot } from "../bindings/Snapshot";
import type { Tool } from "../bindings/Tool";
import type { ToolsStatus } from "../bindings/ToolsStatus";
import type { ViewerOverlay } from "../bindings/ViewerOverlay";
import type { ViewerPointer } from "../bindings/ViewerPointer";

/**
 * A chat failure rather than a document one. The two look alike from here, so the
 * discriminator is the field only one of them has: `retryable` is the whole reason the
 * chat errors are a separate shape.
 */
export function isChatError(value: unknown): value is ChatError {
  return (
    typeof value === "object" &&
    value !== null &&
    "code" in value &&
    "retryable" in value
  );
}

export function isCommandError(value: unknown): value is CommandError {
  return (
    typeof value === "object" &&
    value !== null &&
    "code" in value &&
    "message" in value
  );
}

/** A readable message for anything a command can throw. */
export function errorMessage(error: unknown): string {
  if (isCommandError(error)) return error.message;
  if (error instanceof Error) return error.message;
  return String(error);
}

export const ipc = {
  /** Null until a project is made or opened. */
  snapshot: () => invoke<Snapshot | null>("session_snapshot"),
  toolsStatus: () => invoke<ToolsStatus>("tools_status"),
  newProject: (name: string, format: SequenceFormat) =>
    invoke<Snapshot>("project_new", { name, format }),
  openProject: (path: string) => invoke<Snapshot>("project_open", { path }),
  saveProject: (path: string | null) => invoke<Snapshot>("project_save", { path }),
  edit: (command: EditCommand) => invoke<EditResponse>("edit", { command }),
  undo: () => invoke<Snapshot>("undo"),
  redo: () => invoke<Snapshot>("redo"),
  importMedia: (paths: string[]) => invoke<ImportResponse>("media_import", { paths }),
  // Each export names the sequence it is for: the top bar passes the one on screen,
  // and the bin will pass the one that was right-clicked.
  exportInterchange: (sequenceId: string, format: InterchangeFormat, path: string) =>
    invoke<void>("export_interchange", { sequenceId, format, path }),
  exportPackage: (sequenceId: string, folder: string) =>
    invoke<void>("export_package", { sequenceId, folder }),
  renderStart: (sequenceId: string, path: string, settings: RenderSettings) =>
    invoke<void>("render_start", { sequenceId, path, settings }),
  renderCancel: () => invoke<void>("render_cancel"),
  // The projects this machine has opened, newest first.
  closeProject: () => invoke<void>("project_close"),
  recentsList: () => invoke<Recent[]>("recents_list"),
  recentsForget: (path: string) => invoke<void>("recents_forget", { path }),
  nativeMenuActive: () => invoke<boolean>("native_menu_active"),
  // Native preview surface (ADR-0003). No-ops off macOS.
  viewerSetRect: (x: number, y: number, width: number, height: number) =>
    invoke<void>("viewer_set_rect", { x, y, width, height }),
  viewerSync: (sequenceId: string, frame: number) =>
    invoke<void>("viewer_sync", { sequenceId, frame }),
  viewerSetHidden: (hidden: boolean) => invoke<void>("viewer_set_hidden", { hidden }),
  /** Where to draw the handles, or null to take them away (ADR-0006). */
  viewerSetOverlay: (overlay: ViewerOverlay | null) =>
    invoke<void>("viewer_set_overlay", { overlay }),
  viewerPlay: () => invoke<void>("viewer_play"),
  viewerPause: () => invoke<void>("viewer_pause"),
  viewerSeek: (sequenceId: string, frame: number) =>
    invoke<void>("viewer_seek", { sequenceId, frame }),

  /** What can be done from inside the app, as the catalogue describes it. */
  agentListTools: () => invoke<Tool[]>("agent_list_tools"),
  /**
   * Do one of them. `expectedRevision` says which state the caller was looking at, so
   * an edit aimed at a document that has since moved is refused rather than applied to
   * the wrong one. Leave it out only when the document was read a moment ago.
   */
  agentCallTool: (name: string, args?: unknown, expectedRevision?: number) =>
    invoke<AgentReply>("agent_call_tool", {
      name,
      arguments: args ?? null,
      expectedRevision: expectedRevision ?? null,
    }),

  /** Who is signed in, from what is stored. Answers without touching the network. */
  chatStatus: () => invoke<ChatStatus>("chat_status"),
  /**
   * Sign in, or make an existing sign-in usable again. Opens a browser only when there
   * is nothing left to refresh, so this is also the right call for "it stopped working".
   */
  chatSignIn: () => invoke<ChatStatus>("chat_sign_in"),
  chatSignOut: () => invoke<ChatStatus>("chat_sign_out"),
};

export const events = {
  onJobProgress: (handler: (p: JobProgress) => void): Promise<UnlistenFn> =>
    listen<JobProgress>("job://progress", (e) => handler(e.payload)),
  onJobFinished: (handler: (f: JobFinished) => void): Promise<UnlistenFn> =>
    listen<JobFinished>("job://finished", (e) => handler(e.payload)),
  onMenuAction: (handler: (action: MenuAction) => void): Promise<UnlistenFn> =>
    listen<MenuAction>("menu://action", (e) => handler(e.payload)),
  /** Microseconds from the native player's own clock (ADR-0003). */
  onViewerTime: (handler: (micros: number) => void): Promise<UnlistenFn> =>
    listen<number>("viewer://time", (e) => handler(e.payload)),
  /**
   * Mouse events over the picture. The native surface sits above the webview, so
   * nothing over it is heard unless it is reported (ADR-0006).
   */
  onViewerPointer: (handler: (pointer: ViewerPointer) => void): Promise<UnlistenFn> =>
    listen<ViewerPointer>("viewer://pointer", (e) => handler(e.payload)),
  /**
   * That the document moved, and who moved it. Carries no document: the reply to
   * whoever asked already has the new state, and this goes to every webview.
   */
  onDocumentChanged: (handler: (change: DocumentChanged) => void): Promise<UnlistenFn> =>
    listen<DocumentChanged>("document://changed", (e) => handler(e.payload)),
};
