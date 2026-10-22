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

/**
 * A fixed reply, after a pause long enough to see the waiting state. It answers nothing:
 * the preview has no model, and a canned answer that pretended otherwise would be a
 * worse thing to leave in the window than an honest one.
 */
async function sampleAnswer(message: string): Promise<string> {
  await new Promise((done) => setTimeout(done, 600));
  return `Browser preview: there is no model here, so nothing can answer "${message}". In the desktop app this is where the reply appears.`;
}

export function installMockBackend() {
  const snapshot = sampleSnapshot();
  const tools: ToolsStatus = { ffmpeg: "preview", ffprobe: "preview" };
  let callbackId = 0;

  const handlers: Record<string, (args: Record<string, unknown>) => unknown> = {
    session_snapshot: () => (previewingHome() ? null : snapshot),
    tools_status: () => tools,
    undo: () => snapshot,
    redo: () => snapshot,
    native_menu_active: () => false,
    recents_list: () => sampleRecents(),
    recents_forget: () => null,
    // Answered rather than refused, so the assistant can be styled in a browser the way
    // the rest of the app can. The sample account is as real as the sample project.
    chat_status: () => ({ signedIn: true, email: "you@example.com", canInfer: true }),
    chat_reset: () => null,
    chat_send: (args) => sampleAnswer(String(args.message ?? "")),
  };

  Object.assign(window, {
    __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener() {} },
    __TAURI_INTERNALS__: {
      metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main", windowLabel: "main" } },
      transformCallback: () => ++callbackId,
      convertFileSrc: (path: string) => previewMediaUrls[path] ?? "",
      invoke: async (command: string, args: Record<string, unknown> = {}) => {
        if (command.startsWith("plugin:event|")) return ++callbackId;
        const handler = handlers[command];
        if (handler) return handler(args);
        throw PREVIEW_ERROR;
      },
    },
  });
}
