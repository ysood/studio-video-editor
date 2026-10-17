// Crimson: a saturated red frame around near-black content. Panels and chrome are red
// with dark text; wells (bin lists, inspector groups, dialogs) and the stage (viewer
// frame, timeline) are near-black with light text and a red accent. Built from the
// colors of the October 2026 design reference.

import type { ColorRoles, DeepPartial, Theme } from "../types";
import { graphiteTheme as base } from "./graphite";

const red = {
  signal: "#f8031f", // the frame
  deep: "#cc0018",
  gutter: "#9c010f",
  bright: "#ff1f35", // accent on dark
  brightHover: "#ff5263",
} as const;

const ink = {
  0: "#060607",
  1: "#08090a",
  2: "#0a0a0b",
  3: "#0e1011",
  4: "#141416",
  5: "#17171a",
  6: "#1b1b1e",
  7: "#232327",
  8: "#2c2c31",
} as const;

const dark: DeepPartial<ColorRoles> = {
  surface: {
    base: ink[0],
    panel: ink[2],
    raised: ink[5],
    hover: ink[7],
    input: ink[2],
    overlay: ink[2],
    locked: ink[4],
  },
  border: { strong: ink[8], subtle: ink[6] },
  text: {
    primary: "#f3eeee",
    secondary: "#a8a1a2",
    tertiary: "#6d6768",
    onAccent: "#140003",
    // Weight alone carries a section heading. Accent red here would make the label
    // louder than the data under it, and would stop red meaning "active".
    heading: "#f3eeee",
  },
  accent: { base: red.bright, hover: red.brightHover, wash: "rgba(255, 31, 53, 0.16)" },
  status: { danger: "#ff4d5e" },
};

export const crimsonTheme: Theme = {
  ...base,
  id: "crimson",
  name: "Crimson",
  scheme: "dark",

  color: {
    surface: {
      base: red.gutter,
      gutter: red.gutter,
      panel: red.signal,
      // Dark washes rather than other reds, so controls read clearly on the frame.
      raised: "rgba(20, 0, 3, 0.16)",
      hover: "rgba(20, 0, 3, 0.28)",
      input: "rgba(20, 0, 3, 0.12)",
      overlay: ink[2],
      locked: red.deep,
    },
    border: {
      strong: "rgba(20, 0, 3, 0.55)",
      subtle: "rgba(20, 0, 3, 0.22)",
    },
    text: {
      primary: "#140003",
      secondary: "rgba(20, 0, 3, 0.72)",
      tertiary: "rgba(20, 0, 3, 0.48)",
      onAccent: "#fff4f4",
      onMedia: "#ffffff",
      heading: "rgba(20, 0, 3, 0.72)",
    },
    accent: {
      base: "#140003",
      hover: "#3a0008",
      wash: "rgba(20, 0, 3, 0.14)",
    },
    selection: "#ffffff",
    media: {
      video: "#8d0d1c",
      videoEdge: "#c2283b",
      audio: "#208370",
      audioEdge: "#36a891",
      image: "#5437b9",
      imageEdge: "#7a5cdc",
    },
    status: { danger: "#ffe14d" },
    viewer: { surround: red.signal, matte: "#000000" },
    scrim: "rgba(40, 0, 4, 0.6)",
    shadow: "rgba(0, 0, 0, 0.6)",
  },

  surfaces: {
    chrome: { surface: { base: red.signal } },
    well: dark,
    stage: {
      ...dark,
      surface: { ...dark.surface, base: ink[1], panel: ink[1], raised: ink[3], locked: ink[4] },
    },
  },

  // Two radii, far enough apart to read as a decision: tight on anything small,
  // looser on overlays big enough to carry it. 2, 3 and 4 were indistinguishable.
  radius: { control: 2, panel: 2, dialog: 4, clip: 2, round: 9999 },
};
