// Bins shown in the media panel's folder column. Today they are smart bins derived from
// media kind; user-created bins will need a schema change in studio-core (a `bins` list
// on Project) and will appear here alongside these.

import type { MediaAsset } from "../bindings/MediaAsset";
import type { IconName } from "../ui";

export type BinId = "all" | "footage" | "audio" | "stills" | "offline";

export interface Bin {
  id: BinId;
  name: string;
  icon: IconName;
  media: MediaAsset[];
}

export function smartBins(media: MediaAsset[], offline: ReadonlySet<string>): Bin[] {
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
