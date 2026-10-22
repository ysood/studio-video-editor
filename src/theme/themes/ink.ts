// Ink: the dark half of the pair. A warm near-black ground, one brass accent that means
// time and nothing else, and three hues that only ever appear on media.
//
// The references this came from (a cobalt engraving, a near-white page with clay
// glyphs, three brand cards) agree on one thing more than on any colour: how little
// colour they spend. The page is neutral and the hue is rationed to the few places that
// carry meaning. That is the rule here. The chrome is neutral so it never tints a
// judgement about the picture, and everything saturated is either media or the playhead.

import { brass, clay, cobalt, signal, stone, systemSans, verdigris } from "../primitives";
import type { Theme } from "../types";
import { graphiteTheme as base } from "./graphite";

export const inkTheme: Theme = {
  ...base,
  id: "ink",
  name: "Ink",
  scheme: "dark",

  color: {
    surface: {
      base: stone[950],
      gutter: stone[950],
      panel: stone[900],
      raised: stone[800],
      input: stone[950],
      overlay: stone[850],
      locked: stone[850],
    },
    // Light, and weak: a row only has to separate itself from the one above it.
    state: {
      hover: "rgba(255, 252, 245, 0.05)",
      selected: "rgba(255, 252, 245, 0.1)",
    },
    border: {
      strong: stone[700],
      subtle: stone[800],
    },
    text: {
      primary: stone[50],
      secondary: stone[400],
      tertiary: stone[450],
      onAccent: brass.ink,
      onMedia: "rgba(255, 255, 255, 0.94)",
      heading: stone[400],
    },
    accent: {
      base: brass.base,
      hover: brass.light,
      wash: "rgba(235, 157, 60, 0.14)",
    },
    // Lit from inside: the core sits above the ground and the edge below it, so the
    // letters look pressed into the page rather than printed on it.
    wordmark: { core: stone[850], edge: stone[975] },
    // Selection is not the accent. Brass means time, so what is chosen is said in plain
    // white, and the two never have to be told apart at a glance.
    selection: stone[25],
    media: {
      video: cobalt.base,
      videoEdge: cobalt.edge,
      audio: verdigris.base,
      audioEdge: verdigris.edge,
      image: clay.base,
      imageEdge: clay.edge,
    },
    status: {
      danger: signal.onDark,
    },
    // Neutral and dark around the picture, black behind it. Nothing else may sit here:
    // a tinted surround changes what you think the picture is doing.
    viewer: {
      surround: stone[975],
      matte: stone[1000],
    },
    scrim: "rgba(8, 7, 6, 0.66)",
    shadow: "rgba(0, 0, 0, 0.55)",
  },

  surfaces: {
    // Wells sink below the panel they sit in; the stage sinks further still, so an
    // empty track reads as part of the timeline rather than as the gap around it.
    well: { surface: { panel: stone[950], raised: stone[850], input: stone[975] } },
    stage: {
      surface: { base: stone[975], panel: stone[950], raised: stone[850], locked: stone[800] },
    },
  },

  font: { ...base.font, wordmark: systemSans },

  // Graphite squares its panels off, which suits a theme with no gaps between them.
  // Here a panel is a card lying on the ground, and a card has corners.
  radius: { ...base.radius, panel: 8 },
};
