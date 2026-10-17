// File-level actions that involve native dialogs. Kept out of components so menus,
// buttons and shortcuts all share one implementation.

import { ask, open, save } from "@tauri-apps/plugin-dialog";
import { ipc } from "./ipc";
import { useEditor } from "./store";
import type { SequenceFormat } from "./bindings/SequenceFormat";

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

export async function newProject() {
  const store = useEditor.getState();
  try {
    if (!(await confirmDiscard())) return;
    store.setSnapshot(await ipc.newProject("Untitled", DEFAULT_FORMAT));
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
