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

export type SortKey = "name" | "format" | "duration" | "modified";

export type SortDirection = "asc" | "desc";

export const sortLabels: Record<SortKey, string> = {
  name: "Name",
  format: "Format",
  duration: "Duration",
  modified: "Date modified",
};

/**
 * The direction a column opens in when first chosen. Dates open newest first because
 * that is what "recent" means; everything else opens in its natural ascending order.
 */
export function defaultDirection(key: SortKey): SortDirection {
  return key === "modified" ? "desc" : "asc";
}

/**
 * What choosing a sort key does. Choosing the active key flips its direction, which is
 * what clicking a column header does in both reference editors; choosing another key
 * starts it in its default direction.
 */
export function nextSort(current: { sort: SortKey; direction: SortDirection }, key: SortKey): {
  sort: SortKey;
  direction: SortDirection;
} {
  if (key !== current.sort) return { sort: key, direction: defaultDirection(key) };
  return { sort: key, direction: current.direction === "asc" ? "desc" : "asc" };
}

// Numeric collation so 02 comes before 14, which plain string order gets wrong on the
// numbered files cameras produce. Case is ignored so casing does not split the order.
const byName = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

/**
 * A number to order the metadata column by: picture size for anything with video, else
 * sample rate. Pictures sort above audio, since the column shows one or the other.
 */
function formatWeight(m: MediaAsset): number {
  if (m.video) return m.video.width * m.video.height + 1e12;
  if (m.audio) return m.audio.sampleRate;
  return 0;
}

/** -1, 0 or 1 for two possibly missing numbers, with the missing ones always last. */
function compareNullable(a: number | null, b: number | null, sign: number): number {
  if (a === null || b === null) return a === b ? 0 : a === null ? 1 : -1;
  return (a - b) * sign;
}

/**
 * Order for the bin. Ties fall back to name, ascending whatever the direction, so the
 * result is never arbitrary and a reversed list does not shuffle equal items.
 */
export function sortMedia(
  media: readonly MediaAsset[],
  sort: SortKey,
  direction: SortDirection = defaultDirection(sort),
): MediaAsset[] {
  const sign = direction === "asc" ? 1 : -1;
  const tie = (a: MediaAsset, b: MediaAsset) => byName.compare(a.name, b.name);
  const out = [...media];
  if (sort === "duration") {
    // Stills have no duration. They stay together at the end in either direction rather
    // than mixed in with the shortest clips, which is where you expect to find them.
    out.sort((a, b) => compareNullable(a.durationUs, b.durationUs, sign) || tie(a, b));
  } else if (sort === "modified") {
    out.sort((a, b) => (a.fingerprint.modifiedUnixMs - b.fingerprint.modifiedUnixMs) * sign || tie(a, b));
  } else if (sort === "format") {
    out.sort((a, b) => (formatWeight(a) - formatWeight(b)) * sign || tie(a, b));
  } else {
    out.sort((a, b) => byName.compare(a.name, b.name) * sign);
  }
  return out;
}

/** Whether a name matches what was typed in the search field: case-insensitive substring. */
export function matchesQuery(name: string, query: string): boolean {
  const needle = query.trim().toLowerCase();
  return needle === "" || name.toLowerCase().includes(needle);
}

/** Keeps the items whose name matches the query, in their existing order. */
export function filterByName<T extends { name: string }>(items: readonly T[], query: string): T[] {
  return items.filter((item) => matchesQuery(item.name, query));
}

export interface Bin {
  id: BinId;
  name: string;
  icon: IconName;
  media: MediaAsset[];
}

export function smartBins(
  media: MediaAsset[],
  offline: ReadonlySet<string>,
  sort: SortKey = "name",
  direction: SortDirection = defaultDirection(sort),
): Bin[] {
  // Sort once up front so every bin shows the same order.
  media = sortMedia(media, sort, direction);
  const bins: Bin[] = [
    { id: "all", name: "All", icon: "stack", media },
    { id: "footage", name: "Footage", icon: "video", media: media.filter((m) => m.kind === "video") },
    { id: "audio", name: "Audio", icon: "audio", media: media.filter((m) => m.kind === "audio") },
    { id: "stills", name: "Stills", icon: "image", media: media.filter((m) => m.kind === "image") },
  ];
  const missing = media.filter((m) => offline.has(m.id));
  if (missing.length > 0) bins.push({ id: "offline", name: "Offline", icon: "warning", media: missing });
  return bins;
}
