import { describe, expect, it } from "vitest";
import type { Transform } from "./bindings/Transform";
import { IDENTITY, MILLI, isIdentity, layerStyle, milli, units } from "./transform";

const FRAME = { width: 1920, height: 1080 };

const style = (patch: Partial<Transform>) =>
  layerStyle({ ...IDENTITY, ...patch }, FRAME.width, FRAME.height);

describe("units", () => {
  it("reads thousandths as the unit they count", () => {
    expect(units(MILLI)).toBe(1);
    expect(units(-500)).toBe(-0.5);
    expect(milli(0.25)).toBe(250);
  });

  it("knows the identity", () => {
    expect(isIdentity(IDENTITY)).toBe(true);
    expect(isIdentity({ ...IDENTITY, opacity: 999 })).toBe(false);
  });
});

describe("layerStyle", () => {
  it("leaves an untransformed clip where it is", () => {
    const s = style({});
    expect(s.transform).toBe("translate(0%, 0%) rotate(0deg) scale(1, 1)");
    expect(s.transformOrigin).toBe("50% 50%");
    expect(s.opacity).toBe(1);
  });

  it("turns position into a percentage of the frame", () => {
    // 192 px of a 1920 px frame is a tenth of it.
    const s = style({ position: { x: 192 * MILLI, y: -108 * MILLI } });
    expect(s.transform).toBe("translate(10%, -10%) rotate(0deg) scale(1, 1)");
  });

  it("pivots on the anchor and takes it back off the translation", () => {
    // position places the anchor, but transform-origin is a pivot only, so the anchor
    // has to come off the translation for the two to agree.
    const s = style({ anchor: { x: 192 * MILLI, y: 0 } });
    expect(s.transformOrigin).toBe("60% 50%");
    expect(s.transform).toBe("translate(-10%, 0%) rotate(0deg) scale(1, 1)");
  });

  it("writes a negative origin as one signed percentage", () => {
    // calc(50% + -10%) is not valid CSS; a single percentage always is.
    const s = style({ anchor: { x: -192 * MILLI, y: 0 } });
    expect(s.transformOrigin).toBe("40% 50%");
  });

  it("carries scale, rotation and opacity", () => {
    const s = style({
      scale: { x: MILLI / 2, y: MILLI / 4 },
      rotation: 30 * MILLI,
      opacity: MILLI / 2,
    });
    expect(s.transform).toBe("translate(0%, 0%) rotate(30deg) scale(0.5, 0.25)");
    expect(s.opacity).toBe(0.5);
  });
});
