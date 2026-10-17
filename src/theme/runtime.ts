// Applying a theme: write it to CSS variables, load its fonts, and tell non-CSS consumers
// (the timeline canvas) to repaint. `toCssVars` is pure and tested.

import { useSyncExternalStore } from "react";
import { defaultTheme, themes } from "./themes";
import type { Theme, TypeRoleName } from "./types";

const STORAGE_KEY = "studio.theme";

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
  const vars: Record<string, string> = {};

  const walkColors = (prefix: string, node: Record<string, unknown>) => {
    for (const [key, value] of Object.entries(node)) {
      const name = `${prefix}-${kebab(key)}`;
      if (typeof value === "string") vars[name] = value;
      else walkColors(name, value as Record<string, unknown>);
    }
  };
  walkColors("--color", theme.color);

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

  // Timeline sizes the DOM shares with the canvas (header column, ruler corner).
  vars["--timeline-ruler-height"] = px(theme.timeline.rulerHeight);
  vars["--timeline-header-width"] = px(theme.timeline.headerWidth);
  vars["--timeline-header-stripe"] = px(theme.timeline.headerStripe);

  return vars;
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

/** Apply a theme to the document and notify subscribers once its fonts are ready. */
export function applyTheme(theme: Theme): Promise<void> {
  current = theme;
  const root = document.documentElement;
  for (const [name, value] of Object.entries(toCssVars(theme))) root.style.setProperty(name, value);
  root.style.colorScheme = theme.scheme;
  root.dataset.theme = theme.id;
  try {
    localStorage.setItem(STORAGE_KEY, theme.id);
  } catch {
    // Storage may be unavailable; the theme still applies for this session.
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
