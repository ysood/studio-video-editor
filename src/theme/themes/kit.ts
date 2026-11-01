// Kit: Studio in the shared design kit's clothes, so it sits beside the other products
// (LifeOS and the sites built from ~/Developer/templates/design) as one family.
//
// It takes Ink's type, density and timeline metrics, which are about editing video, and
// swaps in only what makes the kit recognisable: pure greys instead of warm stone, indigo
// as the accent, Geist and Geist Mono, and the kit's softer corners. The media hues stay
// Studio's own, because what colour a clip is has nothing to do with the brand.

import geistMonoVariable from "../fonts/GeistMono-Variable.woff2";
import geistVariable from "../fonts/Geist-Variable.woff2";
import { clay, cobalt, geistMono, geistSans, kitGray, kitIndigo, kitRed, verdigris } from "../primitives";
import type { Theme } from "../types";
import { inkTheme as base } from "./ink";

export const kitTheme: Theme = {
  ...base,
  id: "kit",
  name: "Kit",
  scheme: "dark",

  color: {
    surface: {
      base: kitGray[950],
      gutter: kitGray[950],
      panel: kitGray[900],
      raised: kitGray[800],
      input: kitGray[950],
      overlay: kitGray[900],
      locked: kitGray[800],
    },
    state: {
      hover: "rgba(255, 255, 255, 0.06)",
      selected: "rgba(255, 255, 255, 0.1)",
    },
    border: {
      strong: kitGray[700],
      subtle: kitGray[800],
    },
    text: {
      primary: "#ffffff",
      secondary: kitGray[400],
      tertiary: kitGray[500],
      onAccent: "#ffffff",
      onMedia: "rgba(255, 255, 255, 0.94)",
      heading: kitGray[400],
    },
    // Indigo is the kit's brand hue: here it takes the accent's job of time and action
    // (playhead, timecode, primary button, focus), as brass does in Ink.
    accent: {
      base: kitIndigo.base,
      hover: kitIndigo.light,
      wash: "rgba(97, 95, 255, 0.16)",
    },
    wordmark: { core: kitGray[800], edge: "#000000" },
    // Plain white, as in Ink, so what is chosen never reads as the accent.
    selection: "#ffffff",
    media: {
      video: cobalt.base,
      videoEdge: cobalt.edge,
      audio: verdigris.base,
      audioEdge: verdigris.edge,
      image: clay.base,
      imageEdge: clay.edge,
    },
    status: {
      danger: kitRed,
    },
    viewer: {
      surround: kitGray[950],
      matte: "#000000",
    },
    scrim: "rgba(0, 0, 0, 0.66)",
    shadow: "rgba(0, 0, 0, 0.55)",
  },

  surfaces: {
    well: { surface: { panel: kitGray[950], raised: kitGray[800], input: "#000000" } },
    stage: {
      surface: { base: "#000000", panel: kitGray[950], raised: kitGray[800], locked: kitGray[700] },
    },
  },

  font: {
    ...base.font,
    ui: geistSans,
    display: geistSans,
    numeric: geistMono,
    wordmark: geistSans,
    faces: [
      { family: "Geist", src: geistVariable, weight: "100 900" },
      { family: "Geist Mono", src: geistMonoVariable, weight: "100 900" },
    ],
  },

  // The kit's corners: 8 for controls and buttons (square-ish, not Ink's pills), 12 for
  // panels, 16 for dialogs.
  radius: { ...base.radius, control: 8, button: 8, panel: 12, dialog: 16 },
};
