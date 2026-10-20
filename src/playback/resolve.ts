// What plays at a given frame. Mirrors the render graph in
// crates/studio-core/src/export/render.rs: every enabled video clip under the playhead
// is drawn, lowest track first so higher ones sit over it, and every enabled audio clip
// under the playhead is heard.

import type { Clip } from "../bindings/Clip";
import type { MediaAsset } from "../bindings/MediaAsset";
import type { Project } from "../bindings/Project";
import type { Sequence } from "../bindings/Sequence";
import type { Track } from "../bindings/Track";
import { framesToSeconds } from "../format";
import type { Rational } from "../bindings/Rational";

export interface ActiveClip {
  clip: Clip;
  track: Track;
  media: MediaAsset;
  /** Position inside the media file, in seconds. */
  sourceSeconds: number;
}

function active(project: Project, rate: Rational, track: Track, frame: number): ActiveClip | null {
  if (!track.enabled) return null;
  const clip = track.clips.find((c) => c.enabled && frame >= c.start && frame < c.start + c.duration);
  if (!clip) return null;
  const media = project.media.find((m) => m.id === clip.source.mediaId);
  if (!media) return null;
  return {
    clip,
    track,
    media,
    sourceSeconds: framesToSeconds(clip.source.sourceStart + frame - clip.start, rate),
  };
}

/**
 * Every enabled video clip under the playhead, in painting order: the lowest track
 * first, so later ones are drawn over it. A clip no longer hides what is beneath it,
 * because it may be scaled, turned or faded and then the layers below show through.
 */
export function videoStackAt(project: Project, sequence: Sequence, frame: number): ActiveClip[] {
  const rate = sequence.format.frameRate;
  return sequence.tracks
    .filter((t) => t.kind === "video")
    .map((t) => active(project, rate, t, frame))
    .filter((a): a is ActiveClip => a !== null);
}

/** The frontmost video clip under the playhead, or null when nothing is there. */
export function videoAt(project: Project, sequence: Sequence, frame: number): ActiveClip | null {
  const stack = videoStackAt(project, sequence, frame);
  return stack.length === 0 ? null : stack[stack.length - 1]!;
}

export function audioAt(project: Project, sequence: Sequence, frame: number): ActiveClip[] {
  const rate = sequence.format.frameRate;
  return sequence.tracks
    .filter((t) => t.kind === "audio")
    .map((t) => active(project, rate, t, frame))
    .filter((a): a is ActiveClip => a !== null);
}

export function sequenceEnd(sequence: Sequence): number {
  return Math.max(0, ...sequence.tracks.flatMap((t) => t.clips.map((c) => c.start + c.duration)));
}
