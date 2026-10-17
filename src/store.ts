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
import type { TrackChoice } from "./bindings/TrackChoice";
import type { ToolsStatus } from "./bindings/ToolsStatus";
import { errorMessage, ipc } from "./ipc";
import type { Sequence } from "./bindings/Sequence";
import { activeSequence } from "./sequences";

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
  targets: { video?: TrackChoice; audio?: TrackChoice };
  suggested: SequenceFormat;
}

export interface JobState {
  kind: JobKind;
  fraction: number;
}

export const ZOOM_MIN = 0.25; // pixels per frame
export const ZOOM_MAX = 40;

const DEFAULT_VIEW: SequenceView = { playhead: 0, zoom: 4, scrollX: 0 };

/** Where you were in one timeline: kept per sequence, so switching does not move it. */
interface SequenceView {
  playhead: number;
  zoom: number;
  scrollX: number;
}

const ACTIVE_KEY = "studio.activeSequence";

/** Which sequence was open, per project, so reopening lands where you left. */
function rememberedSequence(projectId: string): string | null {
  try {
    const all = JSON.parse(localStorage.getItem(ACTIVE_KEY) ?? "{}") as Record<string, string>;
    return all[projectId] ?? null;
  } catch {
    return null;
  }
}

function rememberSequence(projectId: string, sequenceId: string) {
  try {
    const all = JSON.parse(localStorage.getItem(ACTIVE_KEY) ?? "{}") as Record<string, string>;
    all[projectId] = sequenceId;
    localStorage.setItem(ACTIVE_KEY, JSON.stringify(all));
  } catch {
    // Remembering is a convenience; a project still opens without it.
  }
}

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
  /// Which sequence the window is showing. UI state: switching is not an edit.
  activeSequenceId: string | null;
  /// The playhead, zoom and scroll of every sequence visited this session.
  views: Record<string, SequenceView>;

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
  setActiveSequence: (id: string) => void;
}

/**
 * Which sequence to show after the project changed. Keeps the one on screen if it is
 * still there, otherwise the one this project was last left on, otherwise the first.
 * A sequence can be deleted, or a different project opened, while one is shown.
 */
function openSequence(snapshot: Snapshot, current: string | null): string | null {
  const exists = (id: string | null) =>
    id !== null && snapshot.project.sequences.some((q) => q.id === id);
  if (exists(current)) return current;
  const remembered = rememberedSequence(snapshot.project.id);
  return exists(remembered) ? remembered : null;
}

/** Keep only selected ids that still exist after the project changed. */
function pruneSelection(selection: ClipId[], snapshot: Snapshot): ClipId[] {
  const live = new Set<string>();
  for (const track of snapshot.project.sequences.flatMap((q) => q.tracks)) {
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
  activeSequenceId: null,
  views: {},

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
    set((state) => ({
      snapshot,
      selection: pruneSelection(state.selection, snapshot),
      activeSequenceId: openSequence(snapshot, state.activeSequenceId),
    })),

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
  // Switching timelines keeps each one's playhead, zoom and scroll where it was. A
  // single shared playhead would jump to a frame that means nothing in the sequence
  // being opened, and on a shorter one would sit past the end.
  setActiveSequence: (id) =>
    set((state) => {
      if (!state.snapshot) return { activeSequenceId: id };
      const leaving = activeSequence(state.snapshot.project, state.activeSequenceId).id;
      if (leaving === id) return {};
      const views = {
        ...state.views,
        [leaving]: { playhead: state.playhead, zoom: state.zoom, scrollX: state.scrollX },
      };
      rememberSequence(state.snapshot.project.id, id);
      return { activeSequenceId: id, views, ...(views[id] ?? DEFAULT_VIEW) };
    }),
}));

/**
 * The sequence this window is showing, or null before a project has loaded. Components
 * read the timeline through this rather than reaching into the project, so switching
 * sequences is one place rather than thirty.
 */
export function useActiveSequence(): Sequence | null {
  const snapshot = useEditor((s) => s.snapshot);
  const id = useEditor((s) => s.activeSequenceId);
  return snapshot ? activeSequence(snapshot.project, id) : null;
}
