// Bins shown in the media panel's folder column. Today they are smart bins derived from
// media kind; user-created bins will need a schema change in studio-core (a `bins` list
// on Project) and will appear here alongside these.

import type { MediaAsset } from "../bindings/MediaAsset";
import type { IconName } from "../ui";

export type BinId = "all" | "footage" | "audio" | "stills" | "offline" | "timelines";

/**
 * Sequences live in the bin beside the footage, the way timelines do in Resolve's
 * media pool and sequences do in Premiere's project panel. The entry is separate from
 * the smart bins because it lists sequences rather than media.
 */
export const TIMELINES_BIN: BinId = "timelines";

export type SortKey = "name" | "duration" | "modified";

export const sortLabels: Record<SortKey, string> = {
  name: "Name",
  duration: "Duration",
  modified: "Date modified",
};

// Numeric collation so 02 comes before 14, which plain string order gets wrong on the
// numbered files cameras produce. Case is ignored so casing does not split the order.
const byName = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

/** Order for the bin. Ties fall back to name so the result is never arbitrary. */
export function sortMedia(media: readonly MediaAsset[], sort: SortKey): MediaAsset[] {
  const out = [...media];
  if (sort === "duration") {
    // Stills have no duration. Infinity keeps them together at the end rather than
    // mixed in with the shortest clips, which is where you expect to find them.
    out.sort((a, b) => (a.durationUs ?? Infinity) - (b.durationUs ?? Infinity) || byName.compare(a.name, b.name));
  } else if (sort === "modified") {
    out.sort((a, b) => b.fingerprint.modifiedUnixMs - a.fingerprint.modifiedUnixMs || byName.compare(a.name, b.name));
  } else {
    out.sort((a, b) => byName.compare(a.name, b.name));
  }
  return out;
}

export interface Bin {
  id: BinId;
  name: string;
  icon: IconName;
  media: MediaAsset[];
}

export function smartBins(media: MediaAsset[], offline: ReadonlySet<string>, sort: SortKey = "name"): Bin[] {
  // Sort once up front so every bin shows the same order.
  media = sortMedia(media, sort);
  const bins: Bin[] = [
    { id: "all", name: "All media", icon: "stack", media },
    { id: "footage", name: "Footage", icon: "video", media: media.filter((m) => m.kind === "video") },
    { id: "audio", name: "Audio", icon: "audio", media: media.filter((m) => m.kind === "audio") },
    { id: "stills", name: "Stills", icon: "image", media: media.filter((m) => m.kind === "image") },
  ];
  const missing = media.filter((m) => offline.has(m.id));
  if (missing.length > 0) bins.push({ id: "offline", name: "Offline", icon: "warning", media: missing });
  return bins;
}
