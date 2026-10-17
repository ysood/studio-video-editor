// Applying a theme: write it to CSS variables, load its fonts, and tell non-CSS consumers
// (the timeline canvas) to repaint. `toCssVars` is pure and tested.

import { useSyncExternalStore } from "react";
import { defaultTheme, themes } from "./themes";
import type { ColorRoles, DeepPartial, SurfaceName, Theme, TypeRoleName } from "./types";

export const SURFACES: SurfaceName[] = ["chrome", "panel", "well", "stage"];

// Versioned so choices saved before themes were opt-in do not pin an old default.
const STORAGE_KEY = "studio.theme.v2";
const STYLE_ID = "studio-theme";

type Listener = () => void;

let current: Theme = defaultTheme;
const listeners = new Set<Listener>();

const kebab = (s: string) => s.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);
const px = (n: number) => `${n}px`;

/**
 * Flatten a theme into CSS custom properties. Naming is mechanical so a token's CSS name
 * can always be derived from its path: `color.text.secondary` is `--color-text-secondary`,
 * `size.controlSmall` is `--size-control-small`, `type.body.size` is `--type-body-size`.
 */
export function toCssVars(theme: Theme): Record<string, string> {
  const vars: Record<string, string> = colorVars(theme.color);

  vars["--font-ui"] = theme.font.ui;
  vars["--font-display"] = theme.font.display;
  vars["--font-numeric"] = theme.font.numeric;
  vars["--font-numeric-variant"] = theme.font.numericVariant;

  for (const [role, spec] of Object.entries(theme.type) as [TypeRoleName, Theme["type"][TypeRoleName]][]) {
    const base = `--type-${kebab(role)}`;
    // Composed shorthand so CSS can write `font: var(--type-label)`.
    vars[base] = `${spec.weight} ${px(spec.size)}/${spec.lineHeight} var(--font-${spec.family})`;
    vars[`${base}-family`] = `var(--font-${spec.family})`;
    vars[`${base}-size`] = px(spec.size);
    vars[`${base}-weight`] = String(spec.weight);
    vars[`${base}-line`] = String(spec.lineHeight);
    vars[`${base}-tracking`] = `${spec.tracking}em`;
  }

  for (const [key, value] of Object.entries(theme.space)) vars[`--space-${key}`] = px(value);
  for (const [key, value] of Object.entries(theme.radius)) vars[`--radius-${kebab(key)}`] = px(value);
  for (const [key, value] of Object.entries(theme.border)) vars[`--border-${kebab(key)}`] = px(value);
  for (const [key, value] of Object.entries(theme.size)) vars[`--size-${kebab(key)}`] = px(value);

  vars["--motion-quick"] = `${theme.motion.quick}ms`;
  vars["--motion-base"] = `${theme.motion.base}ms`;
  vars["--motion-easing"] = theme.motion.easing;
  vars["--elevation-dialog"] = theme.elevation.dialog;
  vars["--elevation-popover"] = theme.elevation.popover;

  // Timeline sizes the DOM shares with the canvas (header column, ruler corner).
  vars["--timeline-ruler-height"] = px(theme.timeline.rulerHeight);
  vars["--timeline-header-width"] = px(theme.timeline.headerWidth);
  vars["--timeline-header-stripe"] = px(theme.timeline.headerStripe);

  return vars;
}

/** CSS variables for a set of color roles: `color.text.secondary` is `--color-text-secondary`. */
export function colorVars(color: ColorRoles): Record<string, string> {
  const vars: Record<string, string> = {};
  const walk = (prefix: string, node: Record<string, unknown>) => {
    for (const [key, value] of Object.entries(node)) {
      const name = `${prefix}-${kebab(key)}`;
      if (typeof value === "string") vars[name] = value;
      else walk(name, value as Record<string, unknown>);
    }
  };
  walk("--color", color as unknown as Record<string, unknown>);
  return vars;
}

function merge<T>(base: T, override: DeepPartial<T> | undefined): T {
  if (!override) return base;
  const out = { ...base } as Record<string, unknown>;
  for (const [key, value] of Object.entries(override)) {
    if (value === undefined) continue;
    const current = out[key];
    out[key] =
      typeof value === "object" && value !== null && typeof current === "object" && current !== null
        ? merge(current, value as DeepPartial<typeof current>)
        : value;
  }
  return out as T;
}

/** The colors in effect on a surface: base colors merged with that surface's overrides. */
export function surfaceColors(theme: Theme, surface: SurfaceName): ColorRoles {
  return merge(theme.color, theme.surfaces[surface]);
}

/**
 * The full stylesheet for a theme: every token on :root, then each surface's complete
 * color set under `[data-surface="..."]`. Each surface rule restates every color, so
 * surfaces nest correctly (a panel inside a well gets panel colors back).
 */
export function toStylesheet(theme: Theme): string {
  const block = (selector: string, vars: Record<string, string>) =>
    `${selector} {\n${Object.entries(vars)
      .map(([name, value]) => `  ${name}: ${value};`)
      .join("\n")}\n}`;
  const rules = [block(":root", toCssVars(theme))];
  for (const surface of SURFACES) {
    rules.push(block(`[data-surface="${surface}"]`, colorVars(surfaceColors(theme, surface))));
  }
  return rules.join("\n\n");
}

async function loadFonts(theme: Theme) {
  if (typeof FontFace === "undefined") return;
  await Promise.all(
    theme.font.faces.map(async (face) => {
      try {
        const font = new FontFace(face.family, `url(${face.src})`, {
          weight: face.weight ?? "400",
          style: face.style ?? "normal",
        });
        document.fonts.add(await font.load());
      } catch {
        // A missing font falls back to the rest of the stack; never block the UI.
      }
    }),
  );
}

/**
 * Apply a theme to the document and notify subscribers once its fonts are ready. Pass
 * `remember` when the person chose the theme, so it is restored next launch; otherwise a
 * changed default theme reaches everyone who never picked one.
 */
export function applyTheme(theme: Theme, options: { remember?: boolean } = {}): Promise<void> {
  current = theme;
  const root = document.documentElement;
  let sheet = document.getElementById(STYLE_ID);
  if (!sheet) {
    sheet = document.createElement("style");
    sheet.id = STYLE_ID;
    document.head.appendChild(sheet);
  }
  sheet.textContent = toStylesheet(theme);
  root.style.colorScheme = theme.scheme;
  root.dataset.theme = theme.id;
  if (options.remember) {
    try {
      localStorage.setItem(STORAGE_KEY, theme.id);
    } catch {
      // Storage may be unavailable; the theme still applies for this session.
    }
  }
  listeners.forEach((l) => l());
  return loadFonts(theme).then(() => listeners.forEach((l) => l()));
}

/** The theme saved last time, or the default. */
export function initialTheme(): Theme {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    return themes.find((t) => t.id === saved) ?? defaultTheme;
  } catch {
    return defaultTheme;
  }
}

export function getTheme(): Theme {
  return current;
}

export function subscribeTheme(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Re-renders the caller whenever the theme changes or its fonts finish loading. */
export function useTheme(): Theme {
  return useSyncExternalStore(subscribeTheme, getTheme, getTheme);
}

/**
 * A counter that changes on every theme notification, including font loads that keep
 * the same theme object. Canvas painters depend on it to know when to repaint.
 */
let revision = 0;
subscribeTheme(() => {
  revision += 1;
});
export function useThemeRevision(): number {
  return useSyncExternalStore(subscribeTheme, () => revision, () => revision);
}

/** A CSS font shorthand for a type role, for canvas text. */
export function canvasFont(theme: Theme, role: TypeRoleName): string {
  const spec = theme.type[role];
  return `${spec.weight} ${spec.size}px ${theme.font[spec.family]}`;
}
