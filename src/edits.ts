// Small helpers that turn UI intents into EditCommands. The rules (overwrite, clamping,
// linking) live in Rust; these only pick sensible targets.

import type { MediaAsset } from "./bindings/MediaAsset";
import type { MediaId } from "./bindings/MediaId";
import type { Project } from "./bindings/Project";
import type { SequenceFormat } from "./bindings/SequenceFormat";
import type { TrackId } from "./bindings/TrackId";
import type { TrackKind } from "./bindings/TrackKind";
import { useEditor } from "./store";

/** First unlocked track of a kind in document order (V1, A1). */
export function firstTrack(project: Project, kind: TrackKind): TrackId | null {
  return project.sequence.tracks.find((t) => t.kind === kind && !t.locked)?.id ?? null;
}

export const MEDIA_DRAG_TYPE = "application/x-studio-media";

/**
 * The media being dragged from the bin. Browsers hide drag data until drop, but the
 * timeline needs the media's length to draw a drop preview while hovering.
 */
export const mediaDrag: { current: MediaId | null } = { current: null };

/**
 * The format this clip would want, or null if the sequence already suits it.
 *
 * Only asked for the first clip: the frame rate is fixed once the timeline has clips,
 * because every position in the document is a frame count at that rate. So this is the
 * one moment the sequence can still adopt the footage, and the one moment worth asking.
 */
export function formatMismatch(project: Project, media: MediaAsset): SequenceFormat | null {
  if (!media.video) return null;
  if (project.sequence.tracks.some((t) => t.clips.length > 0)) return null;
  const current = project.sequence.format;
  const { width, height, frameRate } = media.video;
  const matches =
    width === current.width &&
    height === current.height &&
    frameRate.num === current.frameRate.num &&
    frameRate.den === current.frameRate.den;
  return matches ? null : { ...current, width, height, frameRate };
}

/**
 * Place media at `frame`. Video goes to `videoTrack` (or V1) and audio to `audioTrack`
 * (or A1). Call this when the sequence question has already been settled.
 */
export async function placeMedia(
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

/**
 * Place media, pausing first if this is the first clip and it does not match the
 * sequence. Conforming silently is how 25 fps footage ends up losing a frame a second
 * on a 24 fps timeline with nothing on screen to say so.
 */
export async function insertMedia(
  mediaId: MediaId,
  frame: number,
  targets: { video?: TrackId | null; audio?: TrackId | null } = {},
) {
  const store = useEditor.getState();
  const project = store.snapshot?.project;
  if (!project) return;
  const media = project.media.find((m) => m.id === mediaId);
  const suggested = media ? formatMismatch(project, media) : null;
  if (suggested) {
    store.setPendingInsert({ mediaId, frame, targets, suggested });
    return;
  }
  await placeMedia(mediaId, frame, targets);
}
