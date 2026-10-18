// File-level actions that involve native dialogs. Kept out of components so menus,
// buttons and shortcuts all share one implementation.

import { ask, open, save } from "@tauri-apps/plugin-dialog";
import { ipc } from "./ipc";
import { useEditor } from "./store";
import type { MenuAction } from "./bindings/MenuAction";
import type { SequenceFormat } from "./bindings/SequenceFormat";
import { isTextInput } from "./focus";
import { resetLayout } from "./workspace/layout";
import { activeSequence } from "./sequences";

const PROJECT_FILTER = { name: "Studio project", extensions: ["studio"] };

export const MEDIA_EXTENSIONS = [
  "mov", "mp4", "m4v", "mxf", "mkv", "webm", "avi", "mts", "m2ts", "mpg",
  "wav", "aif", "aiff", "mp3", "m4a", "aac", "flac", "ogg",
  "png", "jpg", "jpeg", "tif", "tiff", "webp",
];

export const DEFAULT_FORMAT: SequenceFormat = {
  width: 1920,
  height: 1080,
  frameRate: { num: 24, den: 1 },
  sampleRate: 48000,
};

/** Ask before throwing away unsaved work. Resolves true when it is safe to continue. */
async function confirmDiscard(): Promise<boolean> {
  const snapshot = useEditor.getState().snapshot;
  if (!snapshot?.history.dirty) return true;
  // A fresh project nobody has touched has nothing to lose.
  if (snapshot.path === null && !snapshot.history.canUndo) return true;
  return ask(`"${snapshot.project.name}" has unsaved changes. Discard them?`, {
    title: "Unsaved changes",
    kind: "warning",
    okLabel: "Discard",
    cancelLabel: "Keep editing",
  });
}

function resetView() {
  useEditor.setState({ selection: [], playhead: 0, playing: false, scrollX: 0 });
}

/**
 * Ask for a name and a place rather than making an untitled project on the spot.
 *
 * The dialog writes the file as it creates it, so there is never a project that exists
 * only in memory: see `NewProjectDialog`.
 */
export async function newProject() {
  const store = useEditor.getState();
  try {
    if (!(await confirmDiscard())) return;
    store.setNewProjectOpen(true);
  } catch (error) {
    store.fail(error);
  }
}

/** Put the project away and go back to the home screen. */
export async function closeProject() {
  const store = useEditor.getState();
  try {
    if (!(await confirmDiscard())) return;
    await store.closeProject();
    resetView();
  } catch (error) {
    store.fail(error);
  }
}

export async function openProject() {
  const store = useEditor.getState();
  try {
    if (!(await confirmDiscard())) return;
    const path = await open({ multiple: false, directory: false, filters: [PROJECT_FILTER] });
    if (typeof path !== "string") return;
    store.setSnapshot(await ipc.openProject(path));
    resetView();
    store.notify("info", `Opened ${path}`);
  } catch (error) {
    store.fail(error);
  }
}

export async function saveProject(saveAs = false) {
  const store = useEditor.getState();
  const snapshot = store.snapshot;
  if (!snapshot) return;
  try {
    let path: string | null = saveAs ? null : snapshot.path;
    if (!path) {
      path = await save({
        defaultPath: `${snapshot.project.name}.studio`,
        filters: [PROJECT_FILTER],
      });
      if (!path) return;
    }
    store.setSnapshot(await ipc.saveProject(path));
    store.notify("info", `Saved ${path}`);
  } catch (error) {
    store.fail(error);
  }
}

export async function importMedia() {
  const store = useEditor.getState();
  try {
    const picked = await open({
      multiple: true,
      directory: false,
      filters: [
        { name: "Media", extensions: MEDIA_EXTENSIONS },
        { name: "All files", extensions: ["*"] },
      ],
    });
    const paths = picked === null ? [] : Array.isArray(picked) ? picked : [picked];
    if (paths.length === 0) return;
    const result = await ipc.importMedia(paths);
    store.setSnapshot(result.snapshot);
    const added = result.report.createdMedia.length;
    if (result.failed.length > 0) {
      const names = result.failed.map((f) => f.path.split("/").pop()).join(", ");
      store.notify("error", `Imported ${added}. Could not import ${names}: ${result.failed[0]?.reason}`);
    } else {
      store.notify("info", `Imported ${added} ${added === 1 ? "file" : "files"}`);
    }
  } catch (error) {
    store.fail(error);
  }
}

export type { MenuAction };

/**
 * A menu event that arrives this soon after the keyboard ran the same action is the same
 * key press seen twice (the web view and the native menu can both receive a shortcut).
 */
const DUPLICATE_WINDOW_MS = 300;
const lastFromKeyboard = new Map<MenuAction, number>();

/**
 * The one entry point for app-level actions, shared by the native menu, the in-window
 * File menu, buttons and shortcuts, so each action behaves the same wherever it starts.
 */
export function runAction(action: MenuAction, from: "ui" | "keyboard" | "menu" = "ui") {
  const now = performance.now();
  if (from === "keyboard") lastFromKeyboard.set(action, now);
  if (from === "menu" && now - (lastFromKeyboard.get(action) ?? -Infinity) < DUPLICATE_WINDOW_MS) return;

  const store = useEditor.getState();
  switch (action) {
    case "newProject":
      return void newProject();
    case "openProject":
      return void openProject();
    case "closeProject":
      return void closeProject();
    case "save":
      return void saveProject();
    case "saveAs":
      return void saveProject(true);
    case "importMedia":
      return void importMedia();
    case "export": {
      const project = store.snapshot?.project;
      const hasClips =
        project !== undefined &&
        activeSequence(project, store.activeSequenceId).tracks.some((t) => t.clips.length > 0);
      if (hasClips) store.setExportOpen(true);
      return;
    }
    case "undo":
    case "redo":
      // The native Edit menu owns Cmd+Z, so a focused text field gets its undo from here.
      if (from === "menu" && isTextInput(document.activeElement)) {
        document.execCommand(action);
        return;
      }
      return void (action === "undo" ? store.undo() : store.redo());
    case "resetLayout":
      return resetLayout();
  }
}
