// Browser preview mode. When the UI runs outside Tauri (plain `npm run dev` in a browser,
// or the hosted preview), this stands in for the Rust backend with a fixed sample
// project so the interface can be reviewed and styled without the native app.
// Edits are refused with a clear message; view state (selection, playhead, zoom,
// playback, layout, themes) works normally.

import type { CommandError } from "../bindings/CommandError";
import type { Recent } from "../bindings/Recent";
import type { ToolsStatus } from "../bindings/ToolsStatus";
import { previewMediaUrls, sampleSnapshot } from "./sampleProject";

const PREVIEW_ERROR: CommandError = {
  code: "edit",
  message: "Browser preview: editing, import and export need the desktop app.",
  detail: null,
};

const DAY = 24 * 60 * 60 * 1000;

/**
 * `#home` previews the home screen instead of the editor, by answering that no project
 * is open. The screen is otherwise only reachable by building the desktop app, which is
 * a slow way to look at a layout.
 */
function previewingHome(): boolean {
  return window.location.hash === "#home";
}

function sampleRecents(now = Date.now()): Recent[] {
  return [
    { path: "/Users/you/Movies/Postcards/Postcards Rough Cut.studio", name: "Postcards Rough Cut", openedUnixMs: BigInt(now - 2 * DAY), exists: true },
    { path: "/Users/you/Movies/Dave/Dave Reel v2.studio", name: "Dave Reel v2", openedUnixMs: BigInt(now - 9 * DAY), exists: true },
    { path: "/Volumes/Scratch/Montage 01.studio", name: "Montage 01", openedUnixMs: BigInt(now - 20 * DAY), exists: false },
  ];
}

export function installMockBackend() {
  const snapshot = sampleSnapshot();
  const tools: ToolsStatus = { ffmpeg: "preview", ffprobe: "preview" };
  let callbackId = 0;

  const handlers: Record<string, () => unknown> = {
    session_snapshot: () => (previewingHome() ? null : snapshot),
    tools_status: () => tools,
    undo: () => snapshot,
    redo: () => snapshot,
    native_menu_active: () => false,
    recents_list: () => sampleRecents(),
    recents_forget: () => null,
    // Answered rather than refused so the panel can be styled in a browser. Signing in
    // needs a loopback listener and the system browser, so the panel says as much and
    // offers no button there.
    chat_status: () => ({ signedIn: false, email: null, canInfer: false }),
  };

  Object.assign(window, {
    __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener() {} },
    __TAURI_INTERNALS__: {
      metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main", windowLabel: "main" } },
      transformCallback: () => ++callbackId,
      convertFileSrc: (path: string) => previewMediaUrls[path] ?? "",
      invoke: async (command: string) => {
        if (command.startsWith("plugin:event|")) return ++callbackId;
        const handler = handlers[command];
        if (handler) return handler();
        throw PREVIEW_ERROR;
      },
    },
  });
}
