import { describe, expect, it } from "vitest";
import { canvasFont, surfaceColors, toCssVars, toStylesheet } from "./runtime";
import { themes } from "./themes";

describe("toCssVars", () => {
  const theme = themes[0]!;
  const vars = toCssVars(theme);

  // The values come from the theme rather than being written out here: what is under
  // test is that a token's path becomes its variable name, not what any theme sets.
  it("derives names mechanically from token paths", () => {
    expect(vars["--color-text-secondary"]).toBe(theme.color.text.secondary);
    expect(vars["--color-surface-base"]).toBe(theme.color.surface.base);
    expect(vars["--size-control-small"]).toBe(`${theme.size.controlSmall}px`);
    expect(vars["--type-timecode-display-size"]).toBe(`${theme.type.timecodeDisplay.size}px`);
    expect(vars["--type-body-family"]).toBe("var(--font-ui)");
    expect(vars["--space-4"]).toBe(`${theme.space[4]}px`);
  });

  it("gives the stacking scale to every theme", () => {
    expect(vars["--z-overlay"]).toBe("100");
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

describe("surfaces", () => {
  const base = themes[0]!;
  const themed = {
    ...base,
    surfaces: { well: { surface: { panel: "#000000" }, text: { primary: "#ffffff" } } },
  };

  it("merges a surface's overrides over the base colors", () => {
    const well = surfaceColors(themed, "well");
    expect(well.surface.panel).toBe("#000000");
    expect(well.text.primary).toBe("#ffffff");
    expect(well.surface.raised).toBe(base.color.surface.raised);
  });

  it("restates every color in each surface rule so surfaces nest", () => {
    const css = toStylesheet(themed);
    const wellRule = css.slice(css.indexOf('[data-surface="well"]'));
    const panelRule = css.slice(css.indexOf('[data-surface="panel"]'), css.indexOf('[data-surface="well"]'));
    expect(wellRule).toContain("--color-surface-panel: #000000;");
    expect(panelRule).toContain(`--color-surface-panel: ${base.color.surface.panel};`);
    expect(panelRule).toContain("--color-text-secondary:");
  });
});
