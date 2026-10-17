import { describe, expect, it } from "vitest";
import type { MediaAsset } from "./bindings/MediaAsset";
import type { Project } from "./bindings/Project";
import { formatMismatch } from "./edits";
import { sampleSnapshot } from "./preview/sampleProject";
import { firstSequence } from "./sequences";

/** The sample project with every clip removed, which is when the question is asked. */
function emptyProject(): Project {
  const project = sampleSnapshot().project;
  return {
    ...project,
    sequences: [
      {
        ...firstSequence(project),
        tracks: firstSequence(project).tracks.map((t) => ({ ...t, clips: [] })),
      },
    ],
  };
}

function video(width: number, height: number, num: number, den = 1): MediaAsset {
  return {
    id: "m",
    name: "clip.mov",
    path: "/footage/clip.mov",
    kind: "video",
    durationUs: 1_000_000,
    video: { width, height, frameRate: { num, den }, codec: "h264" },
    audio: null,
    fingerprint: { sizeBytes: 1, modifiedUnixMs: 0 },
  } as unknown as MediaAsset;
}

describe("formatMismatch", () => {
  const project = emptyProject();
  const { width, height, frameRate } = firstSequence(project).format;

  it("says nothing when the clip already matches", () => {
    expect(formatMismatch(project, video(width, height, frameRate.num, frameRate.den))).toBeNull();
  });

  it("suggests the clip's rate when it differs", () => {
    const suggested = formatMismatch(project, video(width, height, 25));
    expect(suggested?.frameRate).toEqual({ num: 25, den: 1 });
  });

  it("suggests the clip's size when it differs", () => {
    const suggested = formatMismatch(project, video(3840, 2160, frameRate.num, frameRate.den));
    expect(suggested).toMatchObject({ width: 3840, height: 2160 });
  });

  it("keeps the rest of the format, so sample rate is not reset", () => {
    const suggested = formatMismatch(project, video(width, height, 25));
    expect(suggested?.sampleRate).toBe(firstSequence(project).format.sampleRate);
  });

  it("stays quiet once the timeline has clips, because the rate is fixed by then", () => {
    const cut = sampleSnapshot().project;
    expect(formatMismatch(cut, video(width, height, 25))).toBeNull();
  });

  it("stays quiet for media with no picture", () => {
    const audioOnly = { ...video(width, height, 25), video: null } as MediaAsset;
    expect(formatMismatch(project, audioOnly)).toBeNull();
  });
});
