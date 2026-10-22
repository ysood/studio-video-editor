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

const THINKING_MS = 600;

/**
 * A plausible model catalogue, shaped like the real one. Sample data in the same spirit
 * as the sample project: enough to lay the picker out against, including a model with a
 * shorter list of levels, since that is the case the layout has to survive.
 */
const sampleModels = [
  {
    slug: "gpt-6.1-sol",
    name: "GPT-6.1-Sol",
    description: "Latest workhorse model for coding and everyday work.",
    contextWindow: 500000,
    defaultEffort: "low",
    efforts: [
      { effort: "low", description: "Fast responses with lighter reasoning" },
      { effort: "medium", description: "Balances speed and reasoning depth for everyday tasks" },
      { effort: "high", description: "Greater reasoning depth for complex problems" },
      { effort: "ultra", description: "Maximum reasoning with automatic task delegation" },
    ],
  },
  {
    slug: "gpt-6-astra",
    name: "GPT-6-Astra",
    description: "Frontier intelligence for the most demanding work.",
    contextWindow: 272000,
    defaultEffort: "medium",
    efforts: [
      { effort: "low", description: "Fast responses with lighter reasoning" },
      { effort: "medium", description: "Balances speed and reasoning depth for everyday tasks" },
      { effort: "high", description: "Greater reasoning depth for complex problems" },
    ],
  },
  {
    slug: "gpt-6-luna",
    name: "GPT-6-Luna",
    description: "Fast and affordable model for easier tasks.",
    contextWindow: 272000,
    defaultEffort: "medium",
    efforts: [{ effort: "medium", description: "Balances speed and reasoning depth" }],
  },
];

let sampleSettings: { model: string | null; effort: string | null } = {
  model: null,
  effort: null,
};

/**
 * A fixed reply, after a pause long enough to see the waiting state. It answers nothing:
 * the preview has no model, and a canned answer that pretended otherwise would be a
 * worse thing to leave in the window than an honest one.
 *
 * Asking for something to be taken away shows the approval card instead, because that
 * is the one part of the panel that cannot be reached by typing an ordinary question
 * and so would otherwise never be seen while it was being designed.
 */
async function sampleTurn(message: string): Promise<unknown> {
  await new Promise((done) => setTimeout(done, THINKING_MS));
  if (/\b(delete|remove|cut|clear)\b/i.test(message)) {
    return {
      kind: "needsApproval",
      tool: "timeline.delete_clips",
      title: "Delete clips",
      description:
        "Remove clips from the timeline. What was there is gone unless the edit is undone.",
      arguments: { sequence: "V1", clips: ["V1#2"] },
    };
  }
  return {
    kind: "answered",
    text: `Browser preview: there is no model here, so nothing can answer "${message}". In the desktop app this is where the reply appears.`,
  };
}

async function sampleDecision(approved: boolean): Promise<unknown> {
  await new Promise((done) => setTimeout(done, THINKING_MS));
  return {
    kind: "answered",
    text: approved
      ? "Browser preview: in the desktop app the clip would be gone from the timeline by now."
      : "Browser preview: nothing was changed.",
  };
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
    chat_send: (args) => sampleTurn(String(args.message ?? "")),
    chat_approve: (args) => sampleDecision(Boolean(args.approved)),
    chat_models: () => sampleModels,
    chat_settings: () => sampleSettings,
    chat_set_settings: (args) => {
      sampleSettings = {
        model: (args.model as string | null) ?? null,
        effort: (args.effort as string | null) ?? null,
      };
      return sampleSettings;
    },
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
