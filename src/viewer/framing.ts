// Pure viewer geometry: where a clip's picture lands on the stage, what a pointer is
// over, and what a drag does to the transform. Kept free of React and of any drawing so
// it is unit tested, exactly as src/timeline/geometry.ts is for timeline drags.
//
// `placePoint` mirrors `placement` in src-tauri/src/viewer.rs and `centre_offset` in
// crates/studio-core/src/export/render.rs. All three say the same thing: scale and
// rotation turn about the anchor, and position places the anchor. The core stays
// authoritative; this only has to agree with it well enough to preview.

import type { MediaAsset } from "../bindings/MediaAsset";
import type { SequenceFormat } from "../bindings/SequenceFormat";
import type { Transform } from "../bindings/Transform";
import { MILLI, milli, units } from "../transform";

export interface Point {
  x: number;
  y: number;
}

export interface Size {
  width: number;
  height: number;
}

/** Corner order: top left, top right, bottom right, bottom left. */
export type Quad = [Point, Point, Point, Point];

/** What a drag is doing. `corner` carries which one, in `Quad` order. */
export type Grab = { kind: "move" } | { kind: "corner"; corner: number };

/** How close to a corner, in stage pixels, still counts as grabbing it. */
export const HANDLE_REACH = 10;

/**
 * The picture's rectangle inside the frame before any transform: scaled to fit without
 * cropping, centred, which is what every backend does first.
 *
 * The media's stored dimensions are what there is to go on here. Footage a camera
 * recorded sideways reports the size it is stored at rather than the size it is shown
 * at, so this box can be wrong for that clip while the picture itself is drawn correctly
 * (ADR-0006). The renderer and both previews read the real size from the decoder.
 */
export function fittedRect(source: Size, frame: Size): Quad {
  const scale = Math.min(frame.width / source.width, frame.height / source.height);
  const width = source.width * scale;
  const height = source.height * scale;
  const left = (frame.width - width) / 2;
  const top = (frame.height - height) / 2;
  return [
    { x: left, y: top },
    { x: left + width, y: top },
    { x: left + width, y: top + height },
    { x: left, y: top + height },
  ];
}

/**
 * Where the transform sends a point of the frame, in frame pixels.
 *
 * Scale and rotation turn about the anchor, then the anchor lands on the centre of the
 * frame plus position. y counts downwards, so a positive rotation is clockwise.
 */
export function placePoint(t: Transform, frame: Size, p: Point): Point {
  const centre = { x: frame.width / 2, y: frame.height / 2 };
  const anchor = { x: centre.x + units(t.anchor.x), y: centre.y + units(t.anchor.y) };
  const scaled = {
    x: (p.x - anchor.x) * units(t.scale.x),
    y: (p.y - anchor.y) * units(t.scale.y),
  };
  const angle = (units(t.rotation) * Math.PI) / 180;
  const [sin, cos] = [Math.sin(angle), Math.cos(angle)];
  return {
    x: centre.x + units(t.position.x) + scaled.x * cos - scaled.y * sin,
    y: centre.y + units(t.position.y) + scaled.x * sin + scaled.y * cos,
  };
}

/** The picture's four corners in frame pixels, after the transform. */
export function clipQuad(t: Transform, source: Size, frame: Size): Quad {
  const fitted = fittedRect(source, frame);
  return fitted.map((corner) => placePoint(t, frame, corner)) as Quad;
}

/**
 * Frame pixels to stage pixels. The stage always keeps the sequence aspect ratio, so one
 * factor covers both axes and nothing has to know how large the panel happens to be.
 */
export function stageScale(frame: Size, stage: Size): number {
  return frame.width === 0 ? 0 : stage.width / frame.width;
}

export function toStage(point: Point, frame: Size, stage: Size): Point {
  const k = stageScale(frame, stage);
  return { x: point.x * k, y: point.y * k };
}

/** A stage-pixel distance back into frame pixels, for turning a drag into a transform. */
export function toFrame(distance: number, frame: Size, stage: Size): number {
  const k = stageScale(frame, stage);
  return k === 0 ? 0 : distance / k;
}

/**
 * What a pointer at `at` (stage pixels) has hold of, or null for nothing.
 *
 * Corners win over the body, so a pointer in the overlap grabs the handle rather than
 * starting a move, which is the only way a small clip can be scaled at all.
 */
export function grabAt(at: Point, quad: Quad, frame: Size, stage: Size): Grab | null {
  const corners = quad.map((c) => toStage(c, frame, stage));
  for (const [index, corner] of corners.entries()) {
    if (Math.hypot(at.x - corner.x, at.y - corner.y) <= HANDLE_REACH) {
      return { kind: "corner", corner: index };
    }
  }
  return inside(at, corners) ? { kind: "move" } : null;
}

/** Winding test, so a rotated clip is still grabbable by its middle. */
function inside(p: Point, quad: Point[]): boolean {
  let sign = 0;
  for (let i = 0; i < quad.length; i++) {
    const a = quad[i]!;
    const b = quad[(i + 1) % quad.length]!;
    const cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
    if (cross === 0) continue;
    const side = cross > 0 ? 1 : -1;
    if (sign === 0) sign = side;
    else if (sign !== side) return false;
  }
  return true;
}

export interface DragStart {
  grab: Grab;
  /** The transform the clip had when the drag began. */
  transform: Transform;
  /** Pointer position on the stage when the drag began. */
  at: Point;
}

export interface DragOptions {
  /** Shift: scale each axis on its own instead of together (ADR-0006). */
  perAxis?: boolean;
}

/**
 * The transform a drag has reached. Pure, so the preview and the committed edit are the
 * same arithmetic, and the core still clamps whatever comes out.
 */
export function dragged(
  start: DragStart,
  pointer: Point,
  source: Size,
  format: SequenceFormat,
  stage: Size,
  options: DragOptions = {},
): Transform {
  const frame = { width: format.width, height: format.height };
  const t = start.transform;
  if (start.grab.kind === "move") {
    return {
      ...t,
      position: {
        x: t.position.x + milli(toFrame(pointer.x - start.at.x, frame, stage)),
        y: t.position.y + milli(toFrame(pointer.y - start.at.y, frame, stage)),
      },
    };
  }

  // Scaling keeps the anchor still and makes the dragged corner follow the pointer, so
  // the ratio is measured from the anchor rather than from the opposite corner.
  const centre = { x: frame.width / 2, y: frame.height / 2 };
  const anchor = {
    x: centre.x + units(t.position.x),
    y: centre.y + units(t.position.y),
  };
  const corner = clipQuad(t, source, frame)[start.grab.corner]!;
  const was = { x: corner.x - anchor.x, y: corner.y - anchor.y };
  const now = {
    x: toFrame(pointer.x, frame, stage) - anchor.x,
    y: toFrame(pointer.y, frame, stage) - anchor.y,
  };

  if (options.perAxis) {
    // Componentwise, in the clip's own unrotated axes, so a turned clip still grows the
    // way its edges run rather than the way the screen does.
    const angle = (-units(t.rotation) * Math.PI) / 180;
    const [sin, cos] = [Math.sin(angle), Math.cos(angle)];
    const un = (v: Point) => ({ x: v.x * cos - v.y * sin, y: v.x * sin + v.y * cos });
    const [a, b] = [un(was), un(now)];
    return {
      ...t,
      scale: {
        x: scaled(t.scale.x, a.x, b.x),
        y: scaled(t.scale.y, a.y, b.y),
      },
    };
  }

  const factor = ratio(Math.hypot(was.x, was.y), Math.hypot(now.x, now.y));
  return { ...t, scale: { x: scaled(t.scale.x, 1, factor), y: scaled(t.scale.y, 1, factor) } };
}

/** A scale value grown by `now / was`, kept positive and within what the core accepts. */
function scaled(value: number, was: number, now: number): number {
  return Math.max(1, Math.round(value * ratio(was, now)));
}

/**
 * `now / was`, guarding the case where the drag began on the anchor itself and there is
 * no distance to take a ratio of.
 */
function ratio(was: number, now: number): number {
  if (Math.abs(was) < 1e-6) return 1;
  return Math.abs(now / was);
}

/** Whether a transform differs enough from another to be worth an edit. */
export function changed(a: Transform, b: Transform): boolean {
  return (
    a.position.x !== b.position.x ||
    a.position.y !== b.position.y ||
    a.scale.x !== b.scale.x ||
    a.scale.y !== b.scale.y ||
    a.rotation !== b.rotation ||
    a.anchor.x !== b.anchor.x ||
    a.anchor.y !== b.anchor.y ||
    a.opacity !== b.opacity
  );
}

/** The source size to fit, or the sequence frame when the media does not say. */
export function sourceSize(media: MediaAsset, format: SequenceFormat): Size {
  const video = media.video;
  if (!video || video.width === 0 || video.height === 0) {
    return { width: format.width, height: format.height };
  }
  return { width: video.width, height: video.height };
}

export { MILLI };
