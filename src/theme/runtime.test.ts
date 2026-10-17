import { describe, expect, it } from "vitest";
import { canvasFont, toCssVars } from "./runtime";
import { themes } from "./themes";

describe("toCssVars", () => {
  const vars = toCssVars(themes[0]!);

  it("derives names mechanically from token paths", () => {
    expect(vars["--color-text-secondary"]).toBeDefined();
    expect(vars["--color-surface-base"]).toBeDefined();
    expect(vars["--size-control-small"]).toBe("22px");
    expect(vars["--type-timecode-display-size"]).toBe("22px");
    expect(vars["--type-body-family"]).toBe("var(--font-ui)");
    expect(vars["--space-4"]).toBe("8px");
  });

  it("gives every theme the same set of variables", () => {
    const names = Object.keys(vars).sort();
    for (const theme of themes) {
      expect(Object.keys(toCssVars(theme)).sort()).toEqual(names);
    }
  });
});

describe("canvasFont", () => {
  it("builds a canvas font shorthand from a type role", () => {
    expect(canvasFont(themes[0]!, "canvas")).toMatch(/^400 11px /);
  });
});
