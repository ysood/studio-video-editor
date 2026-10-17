// Browser preview mode. When the UI runs outside Tauri (plain `npm run dev` in a browser,
// or the hosted preview), this stands in for the Rust backend with a fixed sample
// project so the interface can be reviewed and styled without the native app.
// Edits are refused with a clear message; view state (selection, playhead, zoom,
// playback, layout, themes) works normally.

import type { CommandError } from "../bindings/CommandError";
import type { ToolsStatus } from "../bindings/ToolsStatus";
import { previewMediaUrls, sampleSnapshot } from "./sampleProject";

const PREVIEW_ERROR: CommandError = {
  code: "edit",
  message: "Browser preview: editing, import and export need the desktop app.",
  detail: null,
};

export function installMockBackend() {
  const snapshot = sampleSnapshot();
  const tools: ToolsStatus = { ffmpeg: "preview", ffprobe: "preview" };
  let callbackId = 0;

  const handlers: Record<string, () => unknown> = {
    session_snapshot: () => snapshot,
    tools_status: () => tools,
    undo: () => snapshot,
    redo: () => snapshot,
    native_menu_active: () => false,
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
