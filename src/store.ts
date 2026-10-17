// UI state. The project itself is only ever replaced by a Snapshot from Rust; this store
// adds view state (selection, playhead, zoom) and wraps IPC calls with error reporting.

import { create } from "zustand";
import type { ClipId } from "./bindings/ClipId";
import type { EditCommand } from "./bindings/EditCommand";
import type { EditReport } from "./bindings/EditReport";
import type { JobKind } from "./bindings/JobKind";
import type { MediaId } from "./bindings/MediaId";
import type { SequenceFormat } from "./bindings/SequenceFormat";
import type { Snapshot } from "./bindings/Snapshot";
import type { ToolsStatus } from "./bindings/ToolsStatus";
import { errorMessage, ipc } from "./ipc";
import { firstSequence } from "./sequences";

export interface StatusMessage {
  kind: "info" | "error";
  text: string;
}

/**
 * An insert held back while the user decides whether the sequence should adopt the
 * clip's format. The sequence can only change while the timeline is empty, so this
 * question can only be asked for the first clip.
 */
export interface PendingInsert {
  mediaId: MediaId;
  frame: number;
  targets: { video?: string | null; audio?: string | null };
  suggested: SequenceFormat;
}

export interface JobState {
  kind: JobKind;
  fraction: number;
}

export const ZOOM_MIN = 0.25; // pixels per frame
export const ZOOM_MAX = 40;

interface EditorState {
  snapshot: Snapshot | null;
  tools: ToolsStatus | null;
  selection: ClipId[];
  playhead: number;
  playing: boolean;
  zoom: number;
  scrollX: number;
  status: StatusMessage | null;
  job: JobState | null;
  /** The OS menu bar carries the File and Edit menus (macOS), so the top bar omits its own. */
  nativeMenu: boolean;
  exportOpen: boolean;
  pendingInsert: PendingInsert | null;

  init: () => Promise<void>;
  setSnapshot: (snapshot: Snapshot) => void;
  edit: (command: EditCommand) => Promise<EditReport | null>;
  undo: () => Promise<void>;
  redo: () => Promise<void>;
  select: (ids: ClipId[], mode?: "replace" | "toggle") => void;
  setPlayhead: (frame: number) => void;
  setPlaying: (playing: boolean) => void;
  setZoom: (zoom: number) => void;
  setScrollX: (scrollX: number) => void;
  notify: (kind: StatusMessage["kind"], text: string) => void;
  fail: (error: unknown) => void;
  setJob: (job: JobState | null) => void;
  setExportOpen: (open: boolean) => void;
  setPendingInsert: (pending: PendingInsert | null) => void;
}

/** Keep only selected ids that still exist after the project changed. */
function pruneSelection(selection: ClipId[], snapshot: Snapshot): ClipId[] {
  const live = new Set<string>();
  for (const track of firstSequence(snapshot.project).tracks) {
    for (const clip of track.clips) live.add(clip.id);
  }
  return selection.filter((id) => live.has(id));
}

export const useEditor = create<EditorState>((set, get) => ({
  snapshot: null,
  tools: null,
  selection: [],
  playhead: 0,
  playing: false,
  zoom: 4,
  scrollX: 0,
  status: null,
  job: null,
  nativeMenu: false,
  exportOpen: false,
  pendingInsert: null,

  init: async () => {
    try {
      const [snapshot, tools, nativeMenu] = await Promise.all([
        ipc.snapshot(),
        ipc.toolsStatus(),
        ipc.nativeMenuActive().catch(() => false),
      ]);
      set({ snapshot, tools, nativeMenu });
      if (!tools.ffmpeg || !tools.ffprobe) {
        get().notify("error", "FFmpeg was not found. Install it with: brew install ffmpeg");
      }
    } catch (error) {
      get().fail(error);
    }
  },

  setSnapshot: (snapshot) =>
    set((state) => ({ snapshot, selection: pruneSelection(state.selection, snapshot) })),

  edit: async (command) => {
    try {
      const { snapshot, report } = await ipc.edit(command);
      get().setSnapshot(snapshot);
      return report;
    } catch (error) {
      get().fail(error);
      return null;
    }
  },

  undo: async () => {
    try {
      get().setSnapshot(await ipc.undo());
    } catch (error) {
      get().fail(error);
    }
  },

  redo: async () => {
    try {
      get().setSnapshot(await ipc.redo());
    } catch (error) {
      get().fail(error);
    }
  },

  select: (ids, mode = "replace") =>
    set((state) => {
      if (mode === "replace") return { selection: ids };
      const next = new Set(state.selection);
      for (const id of ids) {
        if (next.has(id)) next.delete(id);
        else next.add(id);
      }
      return { selection: [...next] };
    }),

  setPlayhead: (frame) => set({ playhead: Math.max(0, Math.round(frame)) }),
  setPlaying: (playing) => set({ playing }),
  setZoom: (zoom) => set({ zoom: Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, zoom)) }),
  setScrollX: (scrollX) => set({ scrollX: Math.max(0, scrollX) }),
  notify: (kind, text) => set({ status: { kind, text } }),
  fail: (error) => set({ status: { kind: "error", text: errorMessage(error) } }),
  setJob: (job) => set({ job }),
  setExportOpen: (exportOpen) => set({ exportOpen }),
  setPendingInsert: (pendingInsert) => set({ pendingInsert }),
}));
