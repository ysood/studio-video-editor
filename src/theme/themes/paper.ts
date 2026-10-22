// Paper: the light half of the pair. The same ramp, the same accent hue and the same
// three media colours as Ink, read from the other end.
//
// It only overrides `color` and `surfaces`. Type, spacing, density and the timeline
// metrics come from Ink untouched, which is what makes the two a pair rather than two
// themes that happen to sit in the same list.

import { brass, clay, cobalt, signal, stone, verdigris } from "../primitives";
import type { Theme } from "../types";
import { inkTheme } from "./ink";

export const paperTheme: Theme = {
  ...inkTheme,
  id: "paper",
  name: "Paper",
  scheme: "light",

  color: {
    surface: {
      // Panels are the page; the ground behind them is a shade darker, so they lift
      // the same way they do in Ink rather than dissolving into the window.
      base: stone[100],
      gutter: stone[100],
      panel: stone[25],
      raised: stone[100],
      // Inset rather than brighter: at this end of the ramp there is no room above the
      // panel, so a field has to be a dip in the page.
      input: stone[50],
      overlay: stone[25],
      locked: stone[200],
    },
    // Dark, because the page is light. The same weights as Ink's, mirrored.
    state: {
      hover: "rgba(26, 24, 22, 0.05)",
      selected: "rgba(26, 24, 22, 0.09)",
    },
    border: {
      strong: stone[300],
      subtle: stone[200],
    },
    text: {
      primary: stone[900],
      secondary: stone[600],
      tertiary: stone[550],
      onAccent: stone[25],
      // Clips keep their colour in both themes, so what sits on them does too.
      onMedia: "rgba(255, 255, 255, 0.94)",
      heading: stone[600],
    },
    accent: {
      base: brass.deep,
      hover: brass.deeper,
      wash: "rgba(166, 98, 27, 0.12)",
    },
    wordmark: { core: stone[50], edge: stone[200] },
    selection: stone[900],
    media: {
      video: cobalt.base,
      videoEdge: cobalt.edge,
      audio: verdigris.base,
      audioEdge: verdigris.edge,
      image: clay.base,
      imageEdge: clay.edge,
    },
    status: {
      danger: signal.onLight,
    },
    // The one place Paper refuses to be light. A white surround blows out the picture
    // and a judgement made against it does not survive the grade, so the viewer keeps
    // a mid grey around the frame and black behind it, in both themes.
    viewer: {
      surround: stone[400],
      matte: stone[1000],
    },
    scrim: "rgba(26, 24, 22, 0.38)",
    shadow: "rgba(45, 40, 35, 0.16)",
  },

  surfaces: {
    // Wells and the stage step down from the page, the same direction as in Ink.
    well: { surface: { panel: stone[50], raised: stone[200], input: stone[25] } },
    stage: {
      surface: { base: stone[200], panel: stone[50], raised: stone[200], locked: stone[300] },
    },
  },

  // A light theme cannot carry Ink's shadows: the same spread at the same opacity reads
  // as dirt on the page rather than as height.
  elevation: {
    dialog: "0 24px 48px -12px rgba(45, 40, 35, 0.22), 0 2px 8px rgba(45, 40, 35, 0.1)",
    popover: "0 12px 24px -8px rgba(45, 40, 35, 0.18), 0 1px 3px rgba(45, 40, 35, 0.1)",
  },
};
