// Typed wrappers over the Tauri commands in src-tauri/src/commands.rs. Every type comes
// from src/bindings, which is generated from Rust, so a schema change that is not
// reflected here fails `npm run typecheck`.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { CommandError } from "../bindings/CommandError";
import type { EditCommand } from "../bindings/EditCommand";
import type { EditResponse } from "../bindings/EditResponse";
import type { ImportResponse } from "../bindings/ImportResponse";
import type { InterchangeFormat } from "../bindings/InterchangeFormat";
import type { JobFinished } from "../bindings/JobFinished";
import type { JobProgress } from "../bindings/JobProgress";
import type { MenuAction } from "../bindings/MenuAction";
import type { RenderSettings } from "../bindings/RenderSettings";
import type { SequenceFormat } from "../bindings/SequenceFormat";
import type { Snapshot } from "../bindings/Snapshot";
import type { ToolsStatus } from "../bindings/ToolsStatus";

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
  snapshot: () => invoke<Snapshot>("session_snapshot"),
  toolsStatus: () => invoke<ToolsStatus>("tools_status"),
  newProject: (name: string, format: SequenceFormat) =>
    invoke<Snapshot>("project_new", { name, format }),
  openProject: (path: string) => invoke<Snapshot>("project_open", { path }),
  saveProject: (path: string | null) => invoke<Snapshot>("project_save", { path }),
  edit: (command: EditCommand) => invoke<EditResponse>("edit", { command }),
  undo: () => invoke<Snapshot>("undo"),
  redo: () => invoke<Snapshot>("redo"),
  importMedia: (paths: string[]) => invoke<ImportResponse>("media_import", { paths }),
  exportInterchange: (format: InterchangeFormat, path: string) =>
    invoke<void>("export_interchange", { format, path }),
  exportPackage: (folder: string) => invoke<void>("export_package", { folder }),
  renderStart: (path: string, settings: RenderSettings) =>
    invoke<void>("render_start", { path, settings }),
  renderCancel: () => invoke<void>("render_cancel"),
  nativeMenuActive: () => invoke<boolean>("native_menu_active"),
  // Native preview surface (ADR-0003). No-ops off macOS.
  viewerSetRect: (x: number, y: number, width: number, height: number) =>
    invoke<void>("viewer_set_rect", { x, y, width, height }),
  viewerSync: () => invoke<void>("viewer_sync"),
  viewerPlay: () => invoke<void>("viewer_play"),
  viewerPause: () => invoke<void>("viewer_pause"),
  viewerSeek: (frame: number) => invoke<void>("viewer_seek", { frame }),
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
};
