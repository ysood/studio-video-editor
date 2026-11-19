// Made-up audio peaks for the two places that draw the timeline without a shell behind
// them: the browser preview and the design gallery. Both need an audio clip to look
// like an audio clip, and neither can run FFmpeg.
//
// Deterministic, because a specimen that looked different on every reload would not be
// a specimen of anything: the whole point of the gallery is that a change to the theme
// is the only thing that can change what it shows.

import type { Peaks } from "../bindings/Peaks";
import type { Project } from "../bindings/Project";
import type { Waveform } from "../timeline/peaks";

const BUCKETS_PER_SECOND = 200;

/** Levels for one asset, or null when it has no sound, as the real command answers. */
function levels(project: Project, mediaId: string): Uint8Array | null {
  const asset = project.media.find((media) => media.id === mediaId);
  if (!asset?.audio) return null;
  const count = Math.max(1, Math.round(((asset.durationUs ?? 0) / 1_000_000) * BUCKETS_PER_SECOND));
  const out = new Uint8Array(count);
  let seed = 1;
  for (let i = 0; i < count; i += 1) {
    seed = (seed * 1103515245 + 12345) % 2147483648;
    const seconds = i / BUCKETS_PER_SECOND;
    // A slow envelope with a faster rattle inside it, which is roughly the shape of
    // speech and reads as sound rather than as noise.
    const envelope = 0.45 + 0.35 * Math.sin(seconds * 2.2) * Math.sin(seconds * 0.7);
    const jitter = 0.55 + (0.45 * seed) / 2147483648;
    out[i] = Math.max(0, Math.min(255, Math.round(255 * envelope * jitter)));
  }
  return out;
}

/** What `media_peaks` would have answered, base64 and all. */
export function samplePeaks(project: Project, mediaId: string): Peaks | null {
  const out = levels(project, mediaId);
  if (!out) return null;
  return { bucketsPerSecond: BUCKETS_PER_SECOND, data: btoa(String.fromCharCode(...out)) };
}

/** Peaks for every asset with sound, in the shape the painter takes. */
export function sampleWaveforms(project: Project): Map<string, Waveform | null> {
  const peaks = new Map<string, Waveform | null>();
  for (const media of project.media) {
    const out = levels(project, media.id);
    peaks.set(media.id, out ? { bucketsPerSecond: BUCKETS_PER_SECOND, levels: out } : null);
  }
  return peaks;
}
