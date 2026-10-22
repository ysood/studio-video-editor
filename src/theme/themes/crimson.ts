// Crimson: a saturated red frame around near-black content. Panels and chrome are red
// with dark text; wells (bin lists, inspector groups, dialogs) and the stage (viewer
// frame, timeline) are near-black with light text and a red accent. Built from the
// colors of the October 2026 design reference.

import epikaTrial from "../fonts/epika-supervariable-trial.woff2";
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
    input: ink[2],
    overlay: ink[2],
    locked: ink[4],
  },
  // Wells and the stage are near black, so their interaction fills lighten.
  state: { hover: "rgba(255, 255, 255, 0.05)", selected: "rgba(255, 255, 255, 0.1)" },
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
      raised: "rgba(20, 0, 3, 0.24)",
      input: "rgba(20, 0, 3, 0.12)",
      overlay: ink[2],
      locked: red.deep,
    },
    // The frame is the one light surface in the app, so here the interaction fills
    // darken. Everything else in the theme inherits the lightening pair from `dark`.
    state: { hover: "rgba(20, 0, 3, 0.12)", selected: "rgba(20, 0, 3, 0.22)" },
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
    // Near black at the edges warming toward the deep red in the middle, so the letters
    // look lit from behind the frame rather than stamped on it.
    wordmark: { core: "#4a0010", edge: "#140003" },
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
    // The stage sits darker than a well, because it frames a picture, but base and
    // panel were the same near black and that flattening cost more than it bought: on
    // the timeline that step is the only thing telling an empty track from the dead
    // space around it, and the rows no longer start under the ruler where their
    // position could be taken on trust. The step here is wider than the well's for
    // that reason, and a locked track still sits clear of both.
    stage: {
      ...dark,
      surface: { ...dark.surface, base: ink[0], panel: ink[3], raised: ink[5], locked: ink[6] },
    },
  },

  font: {
    ...base.font,
    // Epika, bundled, for the name alone. A condensed high-contrast serif is the
    // register of a title card rather than an application, and the width axis is what
    // the system Didones could not give. Trial licence: fine for a tool nobody sells,
    // and the stack still falls back through plainer serifs if the file ever goes.
    wordmark: '"Epika", "Didot", "Bodoni 72", "Hoefler Text", Garamond, serif',
    faces: [
      ...base.font.faces,
      { family: "Epika", src: epikaTrial, weight: "200 900", stretch: "50% 125%" },
    ],
  },

  type: {
    ...base.type,
    // ExtraCondensed Medium, which is wdth 50 and wght 500 in this face. A condensed
    // display face is drawn to set tight, so the capitals get only the hair of
    // tracking they need to stop touching, not the wide spacing a Didone would want.
    wordmark: {
      family: "wordmark",
      size: 80,
      weight: 500,
      lineHeight: 1,
      tracking: 0.02,
      variation: '"wdth" 50, "wght" 500',
    },
  },

  // Softened, but still restrained: enough curve to read as deliberate, not enough to
  // turn playful. Each step is far enough from the last to be a real choice, and the
  // radius grows with the surface, so a dialog is rounder than a button.
  radius: { control: 4, button: 4, panel: 6, dialog: 8, clip: 4, round: 9999 },
};
