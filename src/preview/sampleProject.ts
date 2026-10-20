// A fixed sample project for the browser preview and the design gallery. Shaped exactly
// like a real Snapshot from Rust, so the UI renders it with no special cases.

import type { Clip } from "../bindings/Clip";
import type { MediaAsset } from "../bindings/MediaAsset";
import type { Snapshot } from "../bindings/Snapshot";
import type { Track } from "../bindings/Track";
import { IDENTITY } from "../transform";
import boardMix from "./media/board-mix.ogg";
import crowd from "./media/crowd-handheld.webm";
import stage from "./media/stage-wide.webm";
import titleCard from "./media/title-card.png";

let counter = 0;
const id = () => `00000000-0000-4000-8000-${String(++counter).padStart(12, "0")}`;

const rate = { num: 24, den: 1 };
const fingerprint = { sizeBytes: 1, modifiedUnixMs: 0 };

const media: MediaAsset[] = [
  {
    id: id(),
    name: "A001_C004_stage_wide.mov",
    path: "/Volumes/Shoot/A001_C004_stage_wide.mov",
    kind: "video",
    durationUs: 12_000_000,
    video: { width: 3840, height: 2160, frameRate: rate, codec: "prores" },
    audio: { sampleRate: 48000, channels: 2, codec: "pcm_s24le" },
    fingerprint,
  },
  {
    id: id(),
    name: "A002_C011_crowd_handheld.mov",
    path: "/Volumes/Shoot/A002_C011_crowd_handheld.mov",
    kind: "video",
    durationUs: 8_000_000,
    video: { width: 3840, height: 2160, frameRate: rate, codec: "prores" },
    audio: { sampleRate: 48000, channels: 2, codec: "pcm_s24le" },
    fingerprint,
  },
  {
    id: id(),
    name: "board_mix_set1.wav",
    path: "/Volumes/Shoot/board_mix_set1.wav",
    kind: "audio",
    durationUs: 20_000_000,
    video: null,
    audio: { sampleRate: 48000, channels: 2, codec: "pcm_s24le" },
    fingerprint,
  },
  {
    id: id(),
    name: "title_card.png",
    path: "/Volumes/Shoot/title_card.png",
    kind: "image",
    durationUs: null,
    video: { width: 1280, height: 720, frameRate: { num: 25, den: 1 }, codec: "png" },
    audio: null,
    fingerprint,
  },
];

const [stageMedia, crowdMedia, mixMedia, titleMedia] = media as [MediaAsset, MediaAsset, MediaAsset, MediaAsset];

const urls = new Map<string, string>([
  [stageMedia.path, stage],
  [crowdMedia.path, crowd],
  [mixMedia.path, boardMix],
  [titleMedia.path, titleCard],
]);

// Extra bin-only clips so the gallery holds a realistic amount of footage. They reuse
// the sample files.
function extra(name: string, from: MediaAsset, seconds: number): MediaAsset {
  const asset = { ...from, id: id(), name, path: `/Volumes/Shoot/${name}`, durationUs: seconds * 1_000_000 };
  urls.set(asset.path, urls.get(from.path) ?? "");
  return asset;
}
media.push(
  extra("A003_C016_closeup.mov", crowdMedia, 6),
  extra("A006_C018_backstage.mov", stageMedia, 10),
  extra("A005_C021_skyline.mov", stageMedia, 12),
  extra("A006_C003_lights.mov", crowdMedia, 8),
  extra("A007_C028_silhouette.mov", stageMedia, 11),
  extra("crowd_ambience.wav", mixMedia, 20),
);

/** Where the browser preview finds each sample file. */
export const previewMediaUrls: Record<string, string> = Object.fromEntries(urls);

function clip(m: MediaAsset, start: number, duration: number, sourceStart: number, link: string | null = null): Clip {
  return {
    id: id(),
    name: m.name,
    start,
    duration,
    enabled: true,
    link,
    transform: IDENTITY,
    source: { type: "media", mediaId: m.id, sourceStart },
  };
}

function track(kind: "video" | "audio", name: string, clips: Clip[], locked = false): Track {
  return { id: id(), kind, name, enabled: true, locked, clips };
}

const links = [id(), id(), id()];

export function sampleSnapshot(): Snapshot {
  const tracks = [
    track("video", "V1", [
      clip(stageMedia, 0, 96, 24, links[0]),
      clip(crowdMedia, 96, 120, 24, links[1]),
      clip(stageMedia, 216, 144, 120, links[2]),
    ]),
    track("video", "V2", [clip(titleMedia, 12, 72, 0)]),
    track("audio", "A1", [
      clip(stageMedia, 0, 96, 24, links[0]),
      clip(crowdMedia, 96, 120, 24, links[1]),
      clip(stageMedia, 216, 144, 120, links[2]),
    ]),
    track("audio", "A2", [clip(mixMedia, 0, 360, 48)], true),
  ];
  return {
    project: {
      schemaVersion: 3,
      id: id(),
      name: "Sydney tour recap",
      media,
      sequences: [{
        id: id(),
        name: "Sequence 1",
        format: { width: 1920, height: 1080, frameRate: rate, sampleRate: 48000 },
        tracks,
      }],
    },
    history: { canUndo: false, canRedo: false, undoLabel: null, redoLabel: null, dirty: false },
    path: null,
    offlineMedia: [],
  };
}
