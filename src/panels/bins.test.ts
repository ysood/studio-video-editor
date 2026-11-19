import { describe, expect, it } from "vitest";
import { sampleSnapshot } from "../preview/sampleProject";
import type { MediaAsset } from "../bindings/MediaAsset";
import { defaultDirection, filterByName, matchesQuery, nextSort, smartBins, sortMedia } from "./bins";

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

function asset(name: string, durationUs: number | null, modifiedUnixMs = 0, video?: [number, number]): MediaAsset {
  return {
    id: name,
    name,
    path: `/footage/${name}`,
    kind: durationUs === null ? "image" : "video",
    durationUs,
    video: video ? { width: video[0], height: video[1] } : null,
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

describe("sort direction", () => {
  const names = (media: MediaAsset[]) => media.map((m) => m.name);

  it("opens dates newest first and everything else ascending", () => {
    expect(defaultDirection("modified")).toBe("desc");
    expect(defaultDirection("name")).toBe("asc");
    expect(defaultDirection("duration")).toBe("asc");
  });

  it("reverses names when descending", () => {
    const media = ["1.mp4", "10.mp4", "2.mp4"].map((n) => asset(n, 1));
    expect(names(sortMedia(media, "name", "desc"))).toEqual(["10.mp4", "2.mp4", "1.mp4"]);
  });

  it("keeps stills last when duration is descending", () => {
    const media = [asset("still.png", null), asset("long.mov", 9), asset("short.mov", 1)];
    expect(names(sortMedia(media, "duration", "desc"))).toEqual(["long.mov", "short.mov", "still.png"]);
  });

  it("breaks ties by name ascending in either direction", () => {
    const media = [asset("b.mov", 1), asset("a.mov", 1), asset("c.mov", 2)];
    expect(names(sortMedia(media, "duration", "desc"))).toEqual(["c.mov", "a.mov", "b.mov"]);
  });

  it("puts oldest first when date is ascending", () => {
    const media = [asset("new.mov", 1, 9), asset("old.mov", 1, 1)];
    expect(names(sortMedia(media, "modified", "asc"))).toEqual(["old.mov", "new.mov"]);
  });

  it("orders by picture size for the format column", () => {
    const media = [asset("uhd.mov", 1, 0, [3840, 2160]), asset("hd.mov", 1, 0, [1920, 1080])];
    expect(names(sortMedia(media, "format", "asc"))).toEqual(["hd.mov", "uhd.mov"]);
    expect(names(sortMedia(media, "format", "desc"))).toEqual(["uhd.mov", "hd.mov"]);
  });

  it("flips the active key and resets the direction for a new one", () => {
    expect(nextSort({ sort: "name", direction: "asc" }, "name")).toEqual({ sort: "name", direction: "desc" });
    expect(nextSort({ sort: "name", direction: "desc" }, "name")).toEqual({ sort: "name", direction: "asc" });
    expect(nextSort({ sort: "name", direction: "desc" }, "modified")).toEqual({ sort: "modified", direction: "desc" });
    expect(nextSort({ sort: "modified", direction: "asc" }, "duration")).toEqual({ sort: "duration", direction: "asc" });
  });

  it("gives smartBins the requested direction", () => {
    const media = [asset("2.mov", 1), asset("10.mov", 1)];
    expect(names(smartBins(media, new Set(), "name", "desc")[0]!.media)).toEqual(["10.mov", "2.mov"]);
  });
});

describe("filtering by name", () => {
  const media = ["Beach.mov", "city-night.mp4", "BEACH-drone.mov"].map((n) => asset(n, 1));

  it("matches a case-insensitive substring", () => {
    expect(filterByName(media, "beach").map((m) => m.name)).toEqual(["Beach.mov", "BEACH-drone.mov"]);
    expect(filterByName(media, "NIGHT")).toHaveLength(1);
  });

  it("treats an empty or blank query as no filter", () => {
    expect(filterByName(media, "")).toHaveLength(3);
    expect(filterByName(media, "   ")).toHaveLength(3);
  });

  it("ignores whitespace around the query but not inside it", () => {
    expect(matchesQuery("city night", " city ")).toBe(true);
    expect(matchesQuery("citynight", "city night")).toBe(false);
  });

  it("returns nothing when nothing matches, and keeps order otherwise", () => {
    expect(filterByName(media, "zzz")).toEqual([]);
    expect(filterByName(media, ".mov").map((m) => m.name)).toEqual(["Beach.mov", "BEACH-drone.mov"]);
  });
});
