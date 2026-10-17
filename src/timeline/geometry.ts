// Pure timeline geometry: where tracks and clips are drawn, what a pointer is over, and
// how drag previews are constrained. Kept free of React and canvas so it is unit tested.
//
// Trim clamping mirrors `trim_clip` in crates/studio-core/src/edit.rs so the preview
// matches what the core will accept. The core stays authoritative.

import type { Clip } from "../bindings/Clip";
import type { ClipEdge } from "../bindings/ClipEdge";
import type { ClipId } from "../bindings/ClipId";
import type { Project } from "../bindings/Project";
import type { Sequence } from "../bindings/Sequence";
import type { Track } from "../bindings/Track";
import type { TrackId } from "../bindings/TrackId";
import { microsToFrames } from "../format";
import type { Theme } from "../theme";

/** Timeline dimensions come from the theme so density and proportions are themeable. */
export type TimelineMetrics = Theme["timeline"];

export interface View {
  zoom: number; // pixels per frame
  scrollX: number; // pixels
}

export interface Row {
  track: Track;
  y: number;
  height: number;
}

/** Display order: video tracks from the highest down to V1, then audio from A1 down. */
export function layoutRows(sequence: Sequence, m: TimelineMetrics): Row[] {
  const tracks = sequence.tracks;
  const video = tracks.filter((t) => t.kind === "video").reverse();
  const audio = tracks.filter((t) => t.kind === "audio");
  let y = m.rulerHeight;
  return [...video, ...audio].map((track) => {
    const row = { track, y, height: m.rowHeight };
    y += m.rowHeight + m.rowGap;
    return row;
  });
}

export function contentHeight(rows: Row[], m: TimelineMetrics): number {
  const last = rows[rows.length - 1];
  return last ? last.y + last.height + m.rowGap : m.rulerHeight;
}

export const frameToX = (frame: number, view: View) => frame * view.zoom - view.scrollX;
export const xToFrame = (x: number, view: View) => (x + view.scrollX) / view.zoom;
export const clipEnd = (clip: Clip) => clip.start + clip.duration;

export type Hit =
  | { type: "ruler" }
  | { type: "clip"; clip: Clip; track: Track; zone: "body" | "start" | "end" }
  | { type: "track"; track: Track }
  | { type: "none" };

export function rowAt(rows: Row[], y: number): Row | null {
  return rows.find((r) => y >= r.y && y < r.y + r.height) ?? null;
}

export function hitTest(rows: Row[], view: View, x: number, y: number, m: TimelineMetrics): Hit {
  if (y < m.rulerHeight) return { type: "ruler" };
  const row = rowAt(rows, y);
  if (!row) return { type: "none" };
  for (const clip of row.track.clips) {
    const left = frameToX(clip.start, view);
    const right = frameToX(clipEnd(clip), view);
    if (x < left || x >= right) continue;
    // On short clips the edges would cover the whole body; keep a middle to grab.
    const grab = Math.min(m.edgeGrab, (right - left) / 3);
    const zone = x < left + grab ? "start" : x >= right - grab ? "end" : "body";
    return { type: "clip", clip, track: row.track, zone };
  }
  return { type: "track", track: row.track };
}

export function findClip(sequence: Sequence, id: ClipId): { clip: Clip; track: Track } | null {
  for (const track of sequence.tracks) {
    const clip = track.clips.find((c) => c.id === id);
    if (clip) return { clip, track };
  }
  return null;
}

/** The clip and every clip linked to it. Mirrors Project::link_group. */
export function linkGroup(sequence: Sequence, id: ClipId): ClipId[] {
  const found = findClip(sequence, id);
  if (!found) return [];
  const link = found.clip.link;
  if (!link) return [id];
  return sequence.tracks.flatMap((t) => t.clips.filter((c) => c.link === link).map((c) => c.id));
}

/** Frames of media available at the sequence rate, or null for stills. */
export function mediaFrames(project: Project, sequence: Sequence, clip: Clip): number | null {
  const media = project.media.find((m) => m.id === clip.source.mediaId);
  if (!media || media.kind === "image" || media.durationUs === null) return null;
  return microsToFrames(media.durationUs, sequence.format.frameRate);
}

/** Clip edges and the playhead, excluding the clips being dragged. */
export function snapTargets(sequence: Sequence, exclude: Set<ClipId>, playhead: number): number[] {
  const targets = [0, playhead];
  for (const track of sequence.tracks) {
    for (const clip of track.clips) {
      if (exclude.has(clip.id)) continue;
      targets.push(clip.start, clipEnd(clip));
    }
  }
  return targets;
}

/** Nearest target within `threshold` frames of any probe, as a delta to apply. */
export function snapDelta(probes: number[], targets: number[], threshold: number): number {
  let best = 0;
  let bestDistance = threshold + 1;
  for (const probe of probes) {
    for (const target of targets) {
      const distance = Math.abs(target - probe);
      if (distance < bestDistance) {
        bestDistance = distance;
        best = target - probe;
      }
    }
  }
  return bestDistance <= threshold ? best : 0;
}

export interface MovePreview {
  start: number;
  trackId: TrackId;
}

/**
 * Where a moved clip lands: snapped by its start or end, never before zero, and kept on
 * a track of its own kind.
 */
export function previewMove(
  sequence: Sequence,
  clipId: ClipId,
  rawStart: number,
  overTrack: Track | null,
  snap: { enabled: boolean; playhead: number; thresholdFrames: number },
): MovePreview | null {
  const found = findClip(sequence, clipId);
  if (!found) return null;
  const { clip, track } = found;
  let start = Math.round(rawStart);
  if (snap.enabled) {
    const group = new Set(linkGroup(sequence, clipId));
    const targets = snapTargets(sequence, group, snap.playhead);
    start += snapDelta([start, start + clip.duration], targets, snap.thresholdFrames);
  }
  // A linked clip moves with its group, so the earliest member bounds the move.
  const earliest = Math.min(
    ...linkGroup(sequence, clipId).map((id) => findClip(sequence, id)?.clip.start ?? clip.start),
  );
  start = Math.max(start, clip.start - earliest);
  const trackId =
    overTrack && overTrack.kind === track.kind && !overTrack.locked ? overTrack.id : track.id;
  return { start, trackId };
}

/** Where a dragged edge lands, clamped exactly like the core's trim. */
export function previewTrim(
  project: Project,
  sequence: Sequence,
  clipId: ClipId,
  edge: ClipEdge,
  rawFrame: number,
): number | null {
  const group = linkGroup(sequence, clipId);
  if (group.length === 0) return null;
  const members = new Set(group);
  let lower = -Infinity;
  let upper = Infinity;
  for (const id of group) {
    const found = findClip(sequence, id);
    if (!found) return null;
    const { clip, track } = found;
    const others = track.clips.filter((c) => !members.has(c.id));
    if (edge === "start") {
      const neighbourEnd = Math.max(0, ...others.filter((c) => clipEnd(c) <= clip.start).map(clipEnd));
      const mediaStart = clip.start - clip.source.sourceStart;
      lower = Math.max(lower, neighbourEnd, mediaStart, 0);
      upper = Math.min(upper, clipEnd(clip) - 1);
    } else {
      const neighbourStart = Math.min(
        Infinity,
        ...others.filter((c) => c.start >= clipEnd(clip)).map((c) => c.start),
      );
      const total = mediaFrames(project, sequence, clip);
      const mediaEnd = total === null ? Infinity : clip.start + total - clip.source.sourceStart;
      lower = Math.max(lower, clip.start + 1);
      upper = Math.min(upper, neighbourStart, mediaEnd);
    }
  }
  if (lower > upper) return null;
  return Math.min(upper, Math.max(lower, Math.round(rawFrame)));
}

/** Ruler tick spacing in frames: the smallest step at least `minPx` apart. */
export function tickStep(fps: number, zoom: number, minPx: number): number {
  const steps = [1, 2, 5, 10, fps, 2 * fps, 5 * fps, 10 * fps, 30 * fps, 60 * fps, 300 * fps, 600 * fps, 1800 * fps];
  return steps.find((s) => s * zoom >= minPx) ?? steps[steps.length - 1]!;
}
