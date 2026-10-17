// Small helpers that turn UI intents into EditCommands. The rules (overwrite, clamping,
// linking) live in Rust; these only pick sensible targets.

import type { MediaId } from "./bindings/MediaId";
import type { Project } from "./bindings/Project";
import type { TrackId } from "./bindings/TrackId";
import type { TrackKind } from "./bindings/TrackKind";
import { useEditor } from "./store";

/** First unlocked track of a kind in document order (V1, A1). */
export function firstTrack(project: Project, kind: TrackKind): TrackId | null {
  return project.sequence.tracks.find((t) => t.kind === kind && !t.locked)?.id ?? null;
}

export const MEDIA_DRAG_TYPE = "application/x-studio-media";

/**
 * Place media at `frame`. Video goes to `videoTrack` (or V1) and audio to `audioTrack`
 * (or A1); the core links them and overwrites whatever is underneath.
 */
export async function insertMedia(
  mediaId: MediaId,
  frame: number,
  targets: { video?: TrackId | null; audio?: TrackId | null } = {},
) {
  const store = useEditor.getState();
  const project = store.snapshot?.project;
  if (!project) return;
  const report = await store.edit({
    type: "InsertMedia",
    mediaId,
    start: Math.max(0, Math.round(frame)),
    sourceStart: 0,
    duration: null,
    videoTrack: targets.video ?? firstTrack(project, "video"),
    audioTrack: targets.audio ?? firstTrack(project, "audio"),
  });
  if (report && report.createdClips.length > 0) {
    store.select(report.createdClips);
  }
}
