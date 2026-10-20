// Small helpers that turn UI intents into EditCommands. The rules (overwrite, clamping,
// linking) live in Rust; these only pick sensible targets.

import type { EditCommand } from "./bindings/EditCommand";
import type { MediaAsset } from "./bindings/MediaAsset";
import type { MediaId } from "./bindings/MediaId";
import type { SequenceFormat } from "./bindings/SequenceFormat";
import type { TrackChoice } from "./bindings/TrackChoice";
import type { TrackId } from "./bindings/TrackId";
import type { TrackKind } from "./bindings/TrackKind";
import { useEditor } from "./store";
import { activeSequence } from "./sequences";
import type { Sequence } from "./bindings/Sequence";

/** First unlocked track of a kind in document order (V1, A1). */
export function firstTrack(sequence: Sequence, kind: TrackKind): TrackId | null {
  return sequence.tracks.find((t) => t.kind === kind && !t.locked)?.id ?? null;
}

/**
 * Where one stream of `media` goes: the track the caller named, the first free track of
 * that kind, or a new one when the sequence has none to offer.
 *
 * `present` is whether the media carries the stream at all. Saying so explicitly is the
 * point: "there is no sound to place" and "there is sound but no track free" ask for
 * opposite things, and a target of "no track" cannot tell the core which one it meant.
 */
export function trackChoice(
  sequence: Sequence,
  kind: TrackKind,
  present: boolean,
  named?: TrackId | null,
): TrackChoice {
  if (!present) return { type: "skip" };
  const track = named ?? firstTrack(sequence, kind);
  return track ? { type: "track", trackId: track } : { type: "new" };
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
export function formatMismatch(sequence: Sequence, media: MediaAsset): SequenceFormat | null {
  if (!media.video) return null;
  if (sequence.tracks.some((t) => t.clips.length > 0)) return null;
  const current = sequence.format;
  const { width, height, frameRate } = media.video;
  const matches =
    width === current.width &&
    height === current.height &&
    frameRate.num === current.frameRate.num &&
    frameRate.den === current.frameRate.den;
  return matches ? null : { ...current, width, height, frameRate };
}

/**
 * Place media at `frame`. Each stream the media has goes where `targets` says, or to the
 * first free track of its kind. Call this when the sequence question has been settled.
 */
export async function placeMedia(
  mediaId: MediaId,
  frame: number,
  targets: { video?: TrackChoice; audio?: TrackChoice } = {},
) {
  const store = useEditor.getState();
  const project = store.snapshot?.project;
  if (!project) return;
  const sequence = activeSequence(project, store.activeSequenceId);
  const media = project.media.find((m) => m.id === mediaId);
  if (!media) return;
  const report = await store.edit({
    type: "InsertMedia",
    sequenceId: sequence.id,
    mediaId,
    start: Math.max(0, Math.round(frame)),
    sourceStart: 0,
    duration: null,
    videoTrack: targets.video ?? trackChoice(sequence, "video", media.video !== null),
    audioTrack: targets.audio ?? trackChoice(sequence, "audio", media.audio !== null),
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
  targets: { video?: TrackChoice; audio?: TrackChoice } = {},
) {
  const store = useEditor.getState();
  const project = store.snapshot?.project;
  if (!project) return;
  const sequence = activeSequence(project, store.activeSequenceId);
  const media = project.media.find((m) => m.id === mediaId);
  const suggested = media ? formatMismatch(sequence, media) : null;
  if (suggested) {
    store.setPendingInsert({ mediaId, frame, targets, suggested });
    return;
  }
  await placeMedia(mediaId, frame, targets);
}

/** Everything `SetClipsTransform` can change, with anything left out meaning "as it was". */
export type TransformPatch = Partial<Omit<Extract<EditCommand, { type: "SetClipsTransform" }>, "type" | "clipIds">>;

/**
 * Change part of the transform on some clips. The command names every field so that a
 * panel can commit one number at a time, which makes it verbose to write out; this fills
 * in the nulls that mean "leave this one alone".
 */
export function setTransform(clipIds: string[], patch: TransformPatch): EditCommand {
  return {
    type: "SetClipsTransform",
    clipIds,
    position: null,
    scale: null,
    rotation: null,
    anchor: null,
    opacity: null,
    ...patch,
  };
}
