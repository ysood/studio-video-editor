// Audio peaks for the timeline: asked for once per asset, then kept for as long as the
// window is open.
//
// The cache is module level rather than React state because it is not state: it is a
// fact about a file that cannot change while the window is open, since a file replaced
// on disk gets a new fingerprint and so a new cache entry on the Rust side. Keeping it
// here means switching project and coming back does not decode anything again.
//
// A failure is cached as "nothing to draw" alongside a genuine silence. Both mean the
// same thing to the timeline, and caching the failure is what stops a missing FFmpeg
// from being asked about once per repaint.

import { useEffect, useState } from "react";
import type { Project } from "../bindings/Project";
import { ipc } from "../ipc";

/** One asset's sound, ready to draw. */
export interface Waveform {
  bucketsPerSecond: number;
  /** One byte per bucket: the loudest sample in it, 0 silent and 255 full scale. */
  levels: Uint8Array;
}

/**
 * The loudest sample in the stretch of media one pixel column shows, 0 to 255.
 *
 * Two things this has to get right, and both are why it is a function on its own. A
 * clip is a window on its media, not the whole of it, so the column at the clip's left
 * edge reads the bucket at `sourceStart` and not the bucket at zero; drawing from zero
 * would make trimming a clip appear to rescale its waveform instead of sliding it. And
 * where a column covers many buckets, it takes the loudest of them rather than their
 * average, because averaging is what turns a waveform into a grey smear as you zoom
 * out.
 *
 * `offsetPx` is how far into the clip the column is, and `zoom` is pixels per frame.
 */
export function columnLevel(
  wave: Waveform,
  sourceStart: number,
  offsetPx: number,
  zoom: number,
  fps: number,
): number {
  const bucketsPerFrame = wave.bucketsPerSecond / fps;
  const first = Math.max(0, Math.floor((sourceStart + offsetPx / zoom) * bucketsPerFrame));
  const last = Math.min(
    wave.levels.length,
    Math.max(first + 1, Math.ceil((sourceStart + (offsetPx + 1) / zoom) * bucketsPerFrame)),
  );
  let loudest = 0;
  for (let b = first; b < last; b += 1) loudest = Math.max(loudest, wave.levels[b] ?? 0);
  return loudest;
}

/** Null for an asset that has been asked about and has no shape to draw. */
const cache = new Map<string, Waveform | null>();
const asking = new Set<string>();
const listeners = new Set<() => void>();

/**
 * The bytes the shell sent. Exported so the one format seam between Rust and here is
 * tested rather than trusted: base64 that is decoded wrong is still base64, and the
 * waveform it draws would be wrong without anything failing.
 */
export function decodeLevels(data: string): Uint8Array {
  const binary = atob(data);
  const levels = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) levels[i] = binary.charCodeAt(i);
  return levels;
}

/** Peaks for every asset with sound, and a number that changes when more arrive. */
export function usePeaks(project: Project | null): { peaks: Map<string, Waveform | null>; revision: number } {
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    const listener = () => setRevision((n) => n + 1);
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  }, []);

  useEffect(() => {
    if (!project) return;
    for (const media of project.media) {
      // Only what has sound, and only once: a video whose audio was skipped on the
      // timeline still has peaks worth having if it is placed again later.
      if (!media.audio || cache.has(media.id) || asking.has(media.id)) continue;
      asking.add(media.id);
      ipc
        .mediaPeaks(media.id)
        .then((peaks) => {
          cache.set(
            media.id,
            peaks
              ? { bucketsPerSecond: peaks.bucketsPerSecond, levels: decodeLevels(peaks.data) }
              : null,
          );
        })
        .catch(() => cache.set(media.id, null))
        .finally(() => {
          asking.delete(media.id);
          for (const listener of listeners) listener();
        });
    }
  }, [project]);

  return { peaks: cache, revision };
}
