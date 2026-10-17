import { describe, expect, it } from "vitest";
import type { MediaAsset } from "./bindings/MediaAsset";
import type { Project } from "./bindings/Project";
import { formatMismatch, trackChoice } from "./edits";
import { sampleSnapshot } from "./preview/sampleProject";

/** The sample project with every clip removed, which is when the question is asked. */
function emptyProject(): Project {
  const project = sampleSnapshot().project;
  return {
    ...project,
    sequences: [
      {
        ...project.sequences[0]!,
        tracks: project.sequences[0]!.tracks.map((t) => ({ ...t, clips: [] })),
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
  const { width, height, frameRate } = project.sequences[0]!.format;

  it("says nothing when the clip already matches", () => {
    expect(formatMismatch(project.sequences[0]!, video(width, height, frameRate.num, frameRate.den))).toBeNull();
  });

  it("suggests the clip's rate when it differs", () => {
    const suggested = formatMismatch(project.sequences[0]!, video(width, height, 25));
    expect(suggested?.frameRate).toEqual({ num: 25, den: 1 });
  });

  it("suggests the clip's size when it differs", () => {
    const suggested = formatMismatch(project.sequences[0]!, video(3840, 2160, frameRate.num, frameRate.den));
    expect(suggested).toMatchObject({ width: 3840, height: 2160 });
  });

  it("keeps the rest of the format, so sample rate is not reset", () => {
    const suggested = formatMismatch(project.sequences[0]!, video(width, height, 25));
    expect(suggested?.sampleRate).toBe(project.sequences[0]!.format.sampleRate);
  });

  it("stays quiet once the timeline has clips, because the rate is fixed by then", () => {
    const cut = sampleSnapshot().project;
    expect(formatMismatch(cut.sequences[0]!, video(width, height, 25))).toBeNull();
  });

  it("stays quiet for media with no picture", () => {
    const audioOnly = { ...video(width, height, 25), video: null } as MediaAsset;
    expect(formatMismatch(project.sequences[0]!, audioOnly)).toBeNull();
  });
});

describe("trackChoice", () => {
  const sequence = emptyProject().sequences[0]!;
  const audio = sequence.tracks.filter((t) => t.kind === "audio");

  it("skips a stream the media does not carry", () => {
    expect(trackChoice(sequence, "audio", false)).toEqual({ type: "skip" });
  });

  it("skips it even when a track is free, so an empty stream places nothing", () => {
    expect(trackChoice(sequence, "audio", false, audio[0]!.id)).toEqual({ type: "skip" });
  });

  it("takes the track it is handed", () => {
    expect(trackChoice(sequence, "audio", true, audio[1]!.id)).toEqual({
      type: "track",
      trackId: audio[1]!.id,
    });
  });

  it("falls back to the first free track of the kind", () => {
    expect(trackChoice(sequence, "audio", true)).toEqual({ type: "track", trackId: audio[0]!.id });
  });

  it("asks for a new track when every one of the kind is locked", () => {
    const locked = {
      ...sequence,
      tracks: sequence.tracks.map((t) => (t.kind === "audio" ? { ...t, locked: true } : t)),
    };
    // This is the case that used to lose the sound: no track to name, and "no track" read
    // as "there is no sound".
    expect(trackChoice(locked, "audio", true)).toEqual({ type: "new" });
  });
});
