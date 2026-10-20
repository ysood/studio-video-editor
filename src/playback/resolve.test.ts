import { describe, expect, it } from "vitest";
import type { Clip } from "../bindings/Clip";
import type { Project } from "../bindings/Project";
import type { Track } from "../bindings/Track";
import { IDENTITY } from "../transform";
import { audioAt, sequenceEnd, videoAt } from "./resolve";

const clip = (id: string, start: number, duration: number, sourceStart = 0): Clip => ({
  id,
  name: id,
  start,
  duration,
  enabled: true,
  link: null,
  transform: IDENTITY,
  source: { type: "media", mediaId: "m", sourceStart },
});

const track = (id: string, kind: "video" | "audio", clips: Clip[], enabled = true): Track => ({
  id,
  kind,
  name: id,
  enabled,
  locked: false,
  clips,
});

function project(tracks: Track[]): Project {
  return {
    schemaVersion: 1,
    id: "p",
    name: "p",
    media: [
      {
        id: "m",
        name: "m.mov",
        path: "/m.mov",
        kind: "video",
        durationUs: 100_000_000,
        video: { width: 1920, height: 1080, frameRate: { num: 24, den: 1 }, codec: "h264" },
        audio: { sampleRate: 48000, channels: 2, codec: "aac" },
        fingerprint: { sizeBytes: 1, modifiedUnixMs: 0 },
      },
    ],
    sequences: [{
      id: "s",
      name: "s",
      format: { width: 1920, height: 1080, frameRate: { num: 24, den: 1 }, sampleRate: 48000 },
      tracks,
    }],
  };
}

describe("videoAt", () => {
  it("prefers the highest enabled video track", () => {
    const p = project([track("v1", "video", [clip("low", 0, 48)]), track("v2", "video", [clip("high", 24, 24)])]);
    expect(videoAt(p, p.sequences[0]!, 10)?.clip.id).toBe("low");
    expect(videoAt(p, p.sequences[0]!, 30)?.clip.id).toBe("high");
  });

  it("skips disabled tracks", () => {
    const p = project([track("v1", "video", [clip("low", 0, 48)]), track("v2", "video", [clip("high", 0, 48)], false)]);
    expect(videoAt(p, p.sequences[0]!, 10)?.clip.id).toBe("low");
  });

  it("maps the playhead into source seconds", () => {
    const p = project([track("v1", "video", [clip("c", 24, 48, 12)])]);
    expect(videoAt(p, p.sequences[0]!, 36)?.sourceSeconds).toBe(1);
    expect(videoAt(p, p.sequences[0]!, 72)).toBeNull();
  });
});

describe("audioAt", () => {
  it("returns every enabled audio clip under the playhead", () => {
    const p = project([
      track("a1", "audio", [clip("x", 0, 48)]),
      track("a2", "audio", [clip("y", 0, 48)]),
      track("a3", "audio", [clip("z", 0, 48)], false),
    ]);
    expect(audioAt(p, p.sequences[0]!, 5).map((a) => a.clip.id)).toEqual(["x", "y"]);
  });
});

describe("sequenceEnd", () => {
  it("is the end of the last clip", () => {
    expect(sequenceEnd(project([track("v1", "video", [clip("a", 10, 20)])]).sequences[0]!)).toBe(30);
    expect(sequenceEnd(project([]).sequences[0]!)).toBe(0);
  });
});
