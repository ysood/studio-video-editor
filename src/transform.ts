// The transform side of the schema, mirrored for the UI.
//
// The document stores every transform quantity in thousandths of its unit (see
// `Transform` in crates/studio-core/src/model.rs), so the UI converts once here rather
// than scattering /1000 through panels, previews and the canvas.

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
