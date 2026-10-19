import type { Rational } from "./bindings/Rational";

/** Nearest whole frames per second, as used for timecode. Mirrors Rational::timebase. */
export function timebase(rate: Rational): number {
  return Math.max(1, Math.floor((rate.num + Math.floor(rate.den / 2)) / rate.den));
}

/** HH:MM:SS:FF non-drop-frame. Mirrors studio_core::timecode. */
export function timecode(frame: number, rate: Rational): string {
  const fps = timebase(rate);
  const sign = frame < 0 ? "-" : "";
  const f = Math.abs(Math.trunc(frame));
  const ff = f % fps;
  const total = Math.floor(f / fps);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${sign}${pad(Math.floor(total / 3600))}:${pad(Math.floor(total / 60) % 60)}:${pad(total % 60)}:${pad(ff)}`;
}

export function framesToSeconds(frames: number, rate: Rational): number {
  return (frames * rate.den) / rate.num;
}

export function secondsToFrames(seconds: number, rate: Rational): number {
  return Math.floor((seconds * rate.num) / rate.den + 1e-6);
}

/** Media length in sequence frames, or null for stills. Mirrors MediaAsset::frames_at. */
export function microsToFrames(micros: number, rate: Rational): number {
  return Math.floor((micros * rate.num) / (rate.den * 1_000_000));
}

export function formatRate(rate: Rational): string {
  const fps = rate.num / rate.den;
  return Number.isInteger(fps) ? `${fps}` : fps.toFixed(3);
}

export function formatDuration(micros: number | null): string {
  if (micros === null) return "Still";
  const seconds = micros / 1_000_000;
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${String(s).padStart(2, "0")}`;
}

/**
 * How long ago, in the roughest terms that are still true: "2 days ago", "1 week ago".
 *
 * A recents list is scanned, not read, and the exact minute a project was last opened
 * has never answered anyone's question. Rounding down means a thing is never reported as
 * older than it is.
 */
export function timeAgo(unixMs: number, now = Date.now()): string {
  const seconds = Math.max(0, Math.floor((now - unixMs) / 1000));
  // Each step is how many of the current unit make one of the next.
  const steps: [number, string][] = [
    [60, "minute"],
    [60, "hour"],
    [24, "day"],
    [7, "week"],
    [4, "month"],
    [12, "year"],
  ];
  let count = seconds;
  let unit = "second";
  for (const [size, next] of steps) {
    if (count < size) break;
    count = Math.floor(count / size);
    unit = next;
  }
  if (unit === "second" && count < 45) return "just now";
  return `${count} ${unit}${count === 1 ? "" : "s"} ago`;
}
