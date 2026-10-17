import { describe, expect, it } from "vitest";
import { canvasFont, surfaceColors, toCssVars, toStylesheet } from "./runtime";
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
