import { describe, expect, it } from "vitest";
import { sampleSnapshot } from "../preview/sampleProject";
import type { MediaAsset } from "../bindings/MediaAsset";
import { smartBins, sortMedia } from "./bins";

describe("smartBins", () => {
  const media = sampleSnapshot().project.media;

  it("groups media by kind, with All first", () => {
    const bins = smartBins(media, new Set());
    expect(bins.map((b) => b.id)).toEqual(["all", "footage", "audio", "stills"]);
    expect(bins[0]!.media).toHaveLength(media.length);
    const counted = bins.slice(1).reduce((n, b) => n + b.media.length, 0);
    expect(counted).toBe(media.length);
  });

  it("adds an Offline bin only when something is offline", () => {
    const bins = smartBins(media, new Set([media[0]!.id]));
    expect(bins.at(-1)).toMatchObject({ id: "offline" });
    expect(bins.at(-1)!.media).toHaveLength(1);
  });
});

function asset(name: string, durationUs: number | null, modifiedUnixMs = 0): MediaAsset {
  return {
    id: name,
    name,
    path: `/footage/${name}`,
    kind: durationUs === null ? "image" : "video",
    durationUs,
    video: null,
    audio: null,
    fingerprint: { sizeBytes: 1, modifiedUnixMs },
  } as unknown as MediaAsset;
}

describe("sortMedia", () => {
  it("orders numbered names the way a human counts", () => {
    const names = ["10.mp4", "2.mp4", "1.mp4", "14.mp4"].map((n) => asset(n, 1));
    expect(sortMedia(names, "name").map((m) => m.name)).toEqual(["1.mp4", "2.mp4", "10.mp4", "14.mp4"]);
  });

  it("ignores case so casing does not split the order", () => {
    const names = ["b.mov", "A.mov", "a.mov"].map((n) => asset(n, 1));
    expect(sortMedia(names, "name").map((m) => m.name)[2]).toBe("b.mov");
  });

  it("puts stills after clips when sorting by duration", () => {
    const media = [asset("still.png", null), asset("long.mov", 9), asset("short.mov", 1)];
    expect(sortMedia(media, "duration").map((m) => m.name)).toEqual(["short.mov", "long.mov", "still.png"]);
  });

  it("sorts newest first by date, falling back to name on a tie", () => {
    const media = [asset("b.mov", 1, 5), asset("a.mov", 1, 5), asset("new.mov", 1, 9)];
    expect(sortMedia(media, "modified").map((m) => m.name)).toEqual(["new.mov", "a.mov", "b.mov"]);
  });

  it("gives every bin the same order", () => {
    const media = [asset("10.mov", 1), asset("2.mov", 1)];
    const bins = smartBins(media, new Set());
    expect(bins[0]!.media.map((m) => m.name)).toEqual(["2.mov", "10.mov"]);
    expect(bins[1]!.media.map((m) => m.name)).toEqual(["2.mov", "10.mov"]);
  });
});
