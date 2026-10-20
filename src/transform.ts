// The transform side of the schema, mirrored for the UI.
//
// The document stores every transform quantity in thousandths of its unit (see
// `Transform` in crates/studio-core/src/model.rs), so the UI converts once here rather
// than scattering /1000 through panels, previews and the canvas.

import type { CSSProperties } from "react";
import type { Offset } from "./bindings/Offset";
import type { Transform } from "./bindings/Transform";

/** Thousandths in one unit: one pixel, one degree, or a factor of one. */
export const MILLI = 1000;

/** Fitted, centred and opaque: the picture before anyone touched it. */
export const IDENTITY: Transform = {
  position: { x: 0, y: 0 },
  scale: { x: MILLI, y: MILLI },
  rotation: 0,
  anchor: { x: 0, y: 0 },
  opacity: MILLI,
};

/** A thousandths value as the unit it counts. */
export function units(value: number): number {
  return value / MILLI;
}

/** A unit value back into thousandths, as the document stores it. */
export function milli(value: number): number {
  return Math.round(value * MILLI);
}

export function isIdentity(t: Transform): boolean {
  return (
    t.position.x === 0 &&
    t.position.y === 0 &&
    t.scale.x === MILLI &&
    t.scale.y === MILLI &&
    t.rotation === 0 &&
    t.anchor.x === 0 &&
    t.anchor.y === 0 &&
    t.opacity === MILLI
  );
}

export function offsetPx(offset: Offset): { x: number; y: number } {
  return { x: units(offset.x), y: units(offset.y) };
}

/**
 * The transform as CSS, for an element that fills the sequence frame and holds its
 * picture inside it with `object-fit: contain`.
 *
 * The element's box is the frame, and `contain` centres the picture in it, so scaling
 * the box scales the picture about its own centre: offsets are then percentages of the
 * box and need no knowledge of how large the stage happens to be drawn.
 *
 * `transform-origin` is a pivot only, but position places the anchor (Premiere's
 * convention, and what the renderer does), so the anchor is taken back off the
 * translation. The anchor then lands on the centre of the frame plus position whatever
 * the rotation is.
 */
export function layerStyle(t: Transform, width: number, height: number): CSSProperties {
  const across = (value: number) => (units(value) / width) * 100;
  const down = (value: number) => (units(value) / height) * 100;
  const x = across(t.position.x - t.anchor.x);
  const y = down(t.position.y - t.anchor.y);
  return {
    // One signed percentage rather than calc(50% + n%), which is not valid CSS when n
    // is negative.
    transformOrigin: `${50 + across(t.anchor.x)}% ${50 + down(t.anchor.y)}%`,
    transform: `translate(${x}%, ${y}%) rotate(${units(t.rotation)}deg) scale(${units(t.scale.x)}, ${units(t.scale.y)})`,
    opacity: units(t.opacity),
  };
}
