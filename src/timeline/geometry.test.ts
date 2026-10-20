import { describe, expect, it } from "vitest";
import type { Clip } from "../bindings/Clip";
import type { Project } from "../bindings/Project";
import type { Track } from "../bindings/Track";
import {
  contentHeight,
  hitTest,
  layoutRows,
  naturalHeight,
  pendingRows,
  previewMove,
  previewTrim,
  snapDelta,
  tickStep,
} from "./geometry";
import { graphiteTheme } from "../theme/themes/graphite";
import { IDENTITY } from "../transform";

const m = graphiteTheme.timeline;

const MEDIA = "m-1";

function clip(id: string, start: number, duration: number, sourceStart = 0, link: string | null = null): Clip {
  return { id, name: id, start, duration, enabled: true, link, transform: IDENTITY, source: { type: "media", mediaId: MEDIA, sourceStart } };
}

function track(id: string, kind: "video" | "audio", clips: Clip[]): Track {
  return { id, kind, name: id, enabled: true, locked: false, clips };
}

/** 10 s of 24 fps media; V1 and A1 hold a linked pair at 50..90 (source 30) and a clip at 0..20. */
function project(): Project {
  return {
    schemaVersion: 1,
    id: "p",
    name: "Test",
    media: [
      {
        id: MEDIA,
        name: "shot.mov",
        path: "/shot.mov",
        kind: "video",
        durationUs: 10_000_000,
        video: { width: 1920, height: 1080, frameRate: { num: 24, den: 1 }, codec: "h264" },
        audio: { sampleRate: 48000, channels: 2, codec: "aac" },
        fingerprint: { sizeBytes: 1, modifiedUnixMs: 0 },
      },
    ],
    sequences: [{
      id: "s",
      name: "S",
      format: { width: 1920, height: 1080, frameRate: { num: 24, den: 1 }, sampleRate: 48000 },
      tracks: [
        track("v1", "video", [clip("v-a", 0, 20), clip("v-b", 50, 40, 30, "L")]),
        track("v2", "video", []),
        track("a1", "audio", [clip("a-b", 50, 40, 30, "L")]),
      ],
    }],
  };
}

const view = { zoom: 2, scrollX: 0 };

describe("layoutRows", () => {
  const sequence = project().sequences[0]!;

  it("stacks video tracks highest first, then audio", () => {
    const rows = layoutRows(sequence, m);
    expect(rows.map((r) => r.track.id)).toEqual(["v2", "v1", "a1"]);
    expect(rows[0]!.y).toBe(m.rulerHeight);
  });

  it("keeps the rows at the top when they fill the space", () => {
    const rows = layoutRows(sequence, m, naturalHeight(sequence, m));
    expect(rows[0]!.y).toBe(m.rulerHeight);
  });

  it("keeps the rows at the top when they overflow", () => {
    const rows = layoutRows(sequence, m, 10);
    expect(rows[0]!.y).toBe(m.rulerHeight);
  });

  it("holds the video and audio boundary at the middle", () => {
    const available = 600;
    const rows = layoutRows(sequence, m, available);
    const a1 = rows.find((r) => r.track.id === "a1")!;
    const middle = m.rulerHeight + (available - m.rulerHeight) / 2;
    expect(Math.abs(a1.y - middle)).toBeLessThanOrEqual(1);
  });

  it("adds a video track above without moving the tracks already there", () => {
    const available = 600;
    const oneVideo = { ...sequence, tracks: sequence.tracks.filter((t) => t.id !== "v2") };
    const before = layoutRows(oneVideo, m, available);
    const after = layoutRows(sequence, m, available);
    const at = (rows: ReturnType<typeof layoutRows>, id: string) => rows.find((r) => r.track.id === id)!.y;

    expect(at(after, "v1")).toBe(at(before, "v1"));
    expect(at(after, "a1")).toBe(at(before, "a1"));
    expect(at(after, "v2")).toBeLessThan(at(after, "v1"));
  });

  it("gives the boundary up rather than push a track off the top", () => {
    // Enough video to fill well past half the panel.
    const many = {
      ...sequence,
      tracks: [
        ...Array.from({ length: 8 }, (_, i) => track(`v${i + 1}`, "video", [])),
        ...sequence.tracks.filter((t) => t.kind === "audio"),
      ],
    };
    const rows = layoutRows(many, m, 600);
    expect(rows[0]!.y).toBeGreaterThanOrEqual(m.rulerHeight);
    expect(rows[rows.length - 1]!.y + m.rowHeight).toBeLessThanOrEqual(600);
  });
});

describe("hitTest", () => {
  const rows = layoutRows(project().sequences[0]!, m);
  const v1Y = rows[1]!.y + m.rowHeight / 2;

  it("finds clip bodies and edges", () => {
    expect(hitTest(rows, view, 120, v1Y, m)).toMatchObject({ type: "clip", zone: "body" });
    expect(hitTest(rows, view, 101, v1Y, m)).toMatchObject({ type: "clip", zone: "start" });
    expect(hitTest(rows, view, 178, v1Y, m)).toMatchObject({ type: "clip", zone: "end" });
  });

  it("reports the ruler and empty track space", () => {
    expect(hitTest(rows, view, 10, 5, m)).toEqual({ type: "ruler" });
    expect(hitTest(rows, view, 60, v1Y, m)).toMatchObject({ type: "track" });
  });
});

describe("snapDelta", () => {
  it("snaps the nearest probe within the threshold", () => {
    expect(snapDelta([48, 88], [50, 100], 3)).toBe(2);
    expect(snapDelta([40], [50], 3)).toBe(0);
  });
});

describe("previewMove", () => {
  const snapOff = { enabled: false, playhead: 0, thresholdFrames: 0 };

  it("never moves a group before zero", () => {
    expect(previewMove(project().sequences[0]!, "v-b", -30, null, snapOff)).toEqual({ start: 0, trackId: "v1" });
  });

  it("only changes track to one of the same kind", () => {
    const p = project();
    const a1 = p.sequences[0]!.tracks[2]!;
    const v2 = p.sequences[0]!.tracks[1]!;
    expect(previewMove(p.sequences[0]!, "v-b", 60, a1, snapOff)?.trackId).toBe("v1");
    expect(previewMove(p.sequences[0]!, "v-b", 60, v2, snapOff)?.trackId).toBe("v2");
  });

  it("snaps to neighbouring edges", () => {
    const snap = { enabled: true, playhead: 500, thresholdFrames: 4 };
    // Start at 22 is within 4 of the end of v-a (20).
    expect(previewMove(project().sequences[0]!, "v-b", 22, null, snap)?.start).toBe(20);
  });
});

describe("previewTrim", () => {
  it("clamps the start edge to media start and the neighbour", () => {
    // Media allows reaching back to 20; the neighbour also ends at 20.
    expect(previewTrim(project(), project().sequences[0]!, "v-b", "start", 0)).toBe(20);
  });

  it("clamps the end edge to the media end", () => {
    // 240 frames of media, source starts at 30, clip starts at 50: last frame is 260.
    expect(previewTrim(project(), project().sequences[0]!, "a-b", "end", 1000)).toBe(260);
  });

  it("keeps at least one frame", () => {
    expect(previewTrim(project(), project().sequences[0]!, "v-b", "end", 0)).toBe(51);
  });
});

describe("tickStep", () => {
  it("chooses wider steps as the view zooms out", () => {
    expect(tickStep(24, 20, m.tickMinSpacing)).toBe(5);
    expect(tickStep(24, 1, m.tickMinSpacing)).toBe(5 * 24);
  });
});

describe("pendingRows", () => {
  const sequence = project().sequences[0]!;
  const step = m.rowHeight + m.rowGap;
  const available = 600;
  const rows = layoutRows(sequence, m, available);

  it("puts a new video track above every row and a new audio track below", () => {
    const pending = pendingRows(rows, ["video", "audio"], m, available);
    const at = (kind: "video" | "audio") => pending.find((r) => r.track.kind === kind)!;
    expect(at("video").y).toBe(rows[0]!.y - step);
    expect(at("audio").y).toBe(rows[rows.length - 1]!.y + step);
  });

  it("lays one row per kind, however many ghosts ask for it", () => {
    expect(pendingRows(rows, ["audio", "audio"], m, available)).toHaveLength(1);
    expect(pendingRows(rows, [], m, available)).toEqual([]);
  });

  it("leaves out a row with no room for it, rather than drawing over the ruler", () => {
    const tight = layoutRows(sequence, m);
    expect(pendingRows(tight, ["video", "audio"], m, contentHeight(tight, m))).toEqual([]);
  });
});
