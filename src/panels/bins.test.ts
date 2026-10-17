import { describe, expect, it } from "vitest";
import { sampleSnapshot } from "../preview/sampleProject";
import { smartBins } from "./bins";

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
