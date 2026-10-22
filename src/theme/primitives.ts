// Raw values. Themes pick from these; components never import this file.

export const graphite = {
  50: "#e4e4e7",
  300: "#9b9ca3",
  500: "#6c6e75",
  700: "#3a3c42",
  750: "#303237",
  800: "#2d2f33",
  850: "#26282c",
  900: "#1f2023",
  950: "#18191b",
  1000: "#000000",
} as const;

export const steel = { 500: "#4d79a8", 300: "#6e97c4" } as const;
export const sea = { 500: "#3d8a77", 300: "#5fae99" } as const;
export const amber = { 400: "#f6c565", 500: "#f0b545", 950: "#1d1606" } as const;
export const coral = { 500: "#e0645a" } as const;

/**
 * Stone: the neutral ramp Ink and Paper are both built from. It is warm, around hue 35
 * at 6 to 20 percent saturation, because a dead grey reads as a utility and the
 * references this came from (printed paper, ink, brass) are all warm. The saturation is
 * low enough that it never competes with the picture.
 */
export const stone = {
  25: "#fbfaf8",
  50: "#f4f2ee",
  100: "#ebe7e2",
  200: "#ded9d2",
  300: "#c8c2b9",
  400: "#a39c92",
  450: "#918a80",
  500: "#7e776d",
  550: "#6e675e",
  600: "#5d574f",
  700: "#423d37",
  750: "#35312c",
  800: "#2b2724",
  850: "#211e1b",
  900: "#1a1816",
  950: "#121110",
  975: "#0c0b0a",
  1000: "#000000",
} as const;

/**
 * Brass: the one accent, and it means one thing, time and the action that commits it.
 * Hue 32, between the brass on the engraving and the amber in the reference cards. It
 * darkens for Paper because a light amber cannot hold a line on white.
 */
export const brass = {
  light: "#f2b161", // hover on dark
  base: "#eb9d3c",
  deep: "#a6621b", // Paper's base
  deeper: "#8a5015", // hover on light
  ink: "#241806", // text on a brass fill
} as const;

/** The three media hues, one from each reference: cobalt, a sea green, and shoal's clay. */
export const cobalt = { base: "#2f5aa8", edge: "#5b84cd" } as const;
export const verdigris = { base: "#2c7d6c", edge: "#4fa894" } as const;
export const clay = { base: "#a04b35", edge: "#c8745c" } as const;

export const signal = { onDark: "#e2544f", onLight: "#b23a30" } as const;

export const systemSans =
  '-apple-system, BlinkMacSystemFont, "SF Pro Text", "Inter", "Segoe UI", sans-serif';
export const systemMono = 'ui-monospace, "SF Mono", Menlo, Consolas, monospace';
