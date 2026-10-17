// What plays at a given frame. Mirrors the render graph in
// crates/studio-core/src/export/render.rs: the highest enabled video track with an
// enabled clip wins, and every enabled audio clip under the playhead is heard.

import type { Clip } from "../bindings/Clip";
import type { MediaAsset } from "../bindings/MediaAsset";
import type { Project } from "../bindings/Project";
import type { Track } from "../bindings/Track";
import { framesToSeconds } from "../format";
import { firstSequence } from "../sequences";

export interface ActiveClip {
  clip: Clip;
  track: Track;
  media: MediaAsset;
  /** Position inside the media file, in seconds. */
  sourceSeconds: number;
}

function active(project: Project, track: Track, frame: number): ActiveClip | null {
  if (!track.enabled) return null;
  const clip = track.clips.find((c) => c.enabled && frame >= c.start && frame < c.start + c.duration);
  if (!clip) return null;
  const media = project.media.find((m) => m.id === clip.source.mediaId);
  if (!media) return null;
  const rate = firstSequence(project).format.frameRate;
  return {
    clip,
    track,
    media,
    sourceSeconds: framesToSeconds(clip.source.sourceStart + frame - clip.start, rate),
  };
}

export function videoAt(project: Project, frame: number): ActiveClip | null {
  const video = firstSequence(project).tracks.filter((t) => t.kind === "video");
  for (let i = video.length - 1; i >= 0; i--) {
    const hit = active(project, video[i]!, frame);
    if (hit) return hit;
  }
  return null;
}

export function audioAt(project: Project, frame: number): ActiveClip[] {
  return firstSequence(project).tracks
    .filter((t) => t.kind === "audio")
    .map((t) => active(project, t, frame))
    .filter((a): a is ActiveClip => a !== null);
}

export function sequenceEnd(project: Project): number {
  return Math.max(0, ...firstSequence(project).tracks.flatMap((t) => t.clips.map((c) => c.start + c.duration)));
}
