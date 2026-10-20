import { describe, expect, it } from "vitest";
import type { SequenceFormat } from "../bindings/SequenceFormat";
import type { Transform } from "../bindings/Transform";
import { IDENTITY, MILLI } from "../transform";
import { clipQuad, dragged, fittedRect, grabAt, placePoint, toFrame } from "./framing";

const HD: SequenceFormat = { width: 1920, height: 1080, frameRate: { num: 24, den: 1 }, sampleRate: 48000 };
const FRAME = { width: HD.width, height: HD.height };
const FULL = { width: 1920, height: 1080 };
const CENTRE = { x: 960, y: 540 };

const close = (got: { x: number; y: number }, want: { x: number; y: number }) => {
  expect(got.x).toBeCloseTo(want.x, 6);
  expect(got.y).toBeCloseTo(want.y, 6);
};

const to = (patch: Partial<Transform>): Transform => ({ ...IDENTITY, ...patch });

describe("fittedRect", () => {
  it("fills a frame of the same shape", () => {
    expect(fittedRect(FULL, FRAME)[0]).toEqual({ x: 0, y: 0 });
    expect(fittedRect(FULL, FRAME)[2]).toEqual({ x: 1920, y: 1080 });
  });

  it("centres a portrait source between bars", () => {
    // 1080x1920 fitted into 1920x1080 is 607.5 wide, centred.
    const quad = fittedRect({ width: 1080, height: 1920 }, FRAME);
    expect(quad[0].y).toBe(0);
    expect(quad[2].y).toBe(1080);
    expect(quad[0].x).toBeCloseTo((1920 - 607.5) / 2, 6);
  });
});

describe("placePoint", () => {
  it("leaves an untransformed point where it is", () => {
    close(placePoint(IDENTITY, FRAME, CENTRE), CENTRE);
    close(placePoint(IDENTITY, FRAME, { x: 0, y: 0 }), { x: 0, y: 0 });
  });

  it("slides a point by the position", () => {
    const t = to({ position: { x: 40 * MILLI, y: -10 * MILLI } });
    close(placePoint(t, FRAME, CENTRE), { x: 1000, y: 530 });
  });

  it("turns clockwise", () => {
    // A point to the right of the centre ends up below it: y counts downwards.
    const t = to({ rotation: 90 * MILLI });
    close(placePoint(t, FRAME, { x: CENTRE.x + 100, y: CENTRE.y }), { x: 960, y: 640 });
  });

  it("keeps the anchor where position puts it, whatever the rotation", () => {
    // The same invariant export::render and the macOS preview are tested against.
    for (const rotation of [0, 30 * MILLI, -90 * MILLI]) {
      const t = to({
        rotation,
        scale: { x: MILLI / 2, y: MILLI / 2 },
        anchor: { x: 100 * MILLI, y: 0 },
      });
      close(placePoint(t, FRAME, { x: CENTRE.x + 100, y: CENTRE.y }), CENTRE);
    }
  });
});

describe("stage mapping", () => {
  it("turns stage pixels into frame pixels at the stage's own size", () => {
    // The case most likely to be wrong: the viewer is rarely the sequence's size.
    expect(toFrame(100, FRAME, { width: 960, height: 540 })).toBe(200);
    expect(toFrame(100, FRAME, { width: 1920, height: 1080 })).toBe(100);
    expect(toFrame(100, FRAME, { width: 3840, height: 2160 })).toBe(50);
  });
});

describe("dragged", () => {
  const stage = { width: 960, height: 540 };

  it("moves by the pointer, measured in frame pixels", () => {
    // 100 stage pixels on a half-size stage is 200 frame pixels.
    const next = dragged(
      { grab: { kind: "move" }, transform: IDENTITY, at: { x: 0, y: 0 } },
      { x: 100, y: -50 },
      FULL,
      HD,
      stage,
    );
    expect(next.position).toEqual({ x: 200 * MILLI, y: -100 * MILLI });
  });

  it("adds to the position a clip already had", () => {
    const start = to({ position: { x: 30 * MILLI, y: 0 } });
    const next = dragged(
      { grab: { kind: "move" }, transform: start, at: { x: 10, y: 0 } },
      { x: 20, y: 0 },
      FULL,
      HD,
      stage,
    );
    expect(next.position.x).toBe(50 * MILLI);
  });

  it("scales uniformly so the dragged corner follows the pointer", () => {
    // Corner 0 is the top left, 1101.6 frame pixels from the centre. Putting the pointer
    // twice that far doubles the scale.
    const next = dragged(
      { grab: { kind: "corner", corner: 0 }, transform: IDENTITY, at: { x: 0, y: 0 } },
      { x: -960 / 2, y: -540 / 2 },
      FULL,
      HD,
      stage,
    );
    expect(next.scale.x).toBe(2 * MILLI);
    expect(next.scale.y).toBe(2 * MILLI);
  });

  it("scales each axis on its own when asked", () => {
    // Pointer at stage y 0 is frame y 0, which is where the corner already was, so only
    // the width grows: the corner is twice as far from the anchor across.
    const next = dragged(
      { grab: { kind: "corner", corner: 0 }, transform: IDENTITY, at: { x: 0, y: 0 } },
      { x: -960 / 2, y: 0 },
      FULL,
      HD,
      stage,
      { perAxis: true },
    );
    expect(next.scale.x).toBe(2 * MILLI);
    expect(next.scale.y).toBe(MILLI);
  });

  it("never scales to nothing", () => {
    // A corner dragged onto the anchor would be a scale of zero, which no backend draws.
    const next = dragged(
      { grab: { kind: "corner", corner: 0 }, transform: IDENTITY, at: { x: 0, y: 0 } },
      { x: 480, y: 270 },
      FULL,
      HD,
      stage,
    );
    expect(next.scale.x).toBeGreaterThan(0);
  });

  it("leaves everything it was not asked about alone", () => {
    const start = to({ rotation: 30 * MILLI, opacity: 500 });
    const next = dragged(
      { grab: { kind: "move" }, transform: start, at: { x: 0, y: 0 } },
      { x: 10, y: 0 },
      FULL,
      HD,
      stage,
    );
    expect(next.rotation).toBe(30 * MILLI);
    expect(next.opacity).toBe(500);
  });
});

describe("grabAt", () => {
  const stage = { width: 1920, height: 1080 };
  const quad = clipQuad(IDENTITY, FULL, FRAME);

  it("takes a corner over the body", () => {
    expect(grabAt({ x: 2, y: 2 }, quad, FRAME, stage)).toEqual({ kind: "corner", corner: 0 });
    expect(grabAt({ x: 1918, y: 1078 }, quad, FRAME, stage)).toEqual({ kind: "corner", corner: 2 });
  });

  it("moves from the middle", () => {
    expect(grabAt(CENTRE, quad, FRAME, stage)).toEqual({ kind: "move" });
  });

  it("is nothing outside the picture", () => {
    const small = clipQuad({ ...IDENTITY, scale: { x: MILLI / 4, y: MILLI / 4 } }, FULL, FRAME);
    expect(grabAt({ x: 20, y: 20 }, small, FRAME, stage)).toBeNull();
  });

  it("still finds the middle of a turned clip", () => {
    const turned = clipQuad({ ...IDENTITY, rotation: 30 * MILLI }, FULL, FRAME);
    expect(grabAt(CENTRE, turned, FRAME, stage)).toEqual({ kind: "move" });
  });
});
