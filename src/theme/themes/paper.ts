// Paper: light mode. A grey ground, blue cards for the panels that hold controls, and a
// near-black panel for the timeline. Text is white on the blue and on the black, and
// black on the grey, which is the only place the grey is the backdrop for words.
//
// The base colour block below is the blue, because the blue is the "panel" surface and
// most of the app is a panel. Chrome (the bars and tab strips) and the stage (the
// timeline and the viewer) override it.

import { clay, cobalt, verdigris } from "../primitives";
import type { ColorRoles, DeepPartial, Theme } from "../types";
import { inkTheme } from "./ink";

const grey = "#e6e5e9"; // the ground
const blue = "#2554ce"; // primary
const black = "#151414"; // the timeline panel, and text on the grey
const bone = "#f0e3de"; // text on the blue and the black

/** A dark wash, for a lift or a dip on the blue. Only the base block reads it now: no
 *  panel is blue any more, so these are the fallback a surface would land on, not a
 *  colour anything actually paints. */
const onBlue = (a: number) => `rgba(4, 14, 48, ${a})`;
/** The two text colours at an alpha. Borders and dividers are the text they separate,
 *  faded, rather than a grey of their own: that way they are right on every surface. */
const white = (a: number) => `rgba(240, 227, 222, ${a})`;
const ink = (a: number) => `rgba(21, 20, 20, ${a})`;

/** What a hairline between or around anything is worth. */
const HAIRLINE = 0.125;

/** Chrome: the top bar, the status bar and the panel tab strips, on the grey ground. */
const chrome: DeepPartial<ColorRoles> = {
  surface: {
    base: grey,
    panel: grey,
    raised: ink(0.07),
    input: ink(0.05),
    overlay: "#ffffff",
    locked: ink(0.12),
  },
  state: { hover: ink(0.055), selected: ink(0.1) },
  border: { strong: ink(0.25), subtle: ink(HAIRLINE) },
  text: {
    primary: black,
    secondary: ink(0.66),
    tertiary: ink(0.45),
    onAccent: bone,
    heading: ink(0.66),
  },
  // On the grey, the primary blue is the accent: it is the one thing that carries it.
  accent: { base: blue, hover: "#1c44ac", wash: "rgba(37, 84, 206, 0.12)" },
  selection: black,
  status: { danger: "#b4231c" },
};

/** The stage: the timeline and the viewer, the two places that hold picture and time. */
const stage: DeepPartial<ColorRoles> = {
  surface: {
    base: "#0f0f0f",
    panel: black,
    raised: white(0.08),
    input: "#0f0f0f",
    overlay: "#222222",
    locked: "#232323",
  },
  state: { hover: white(0.06), selected: white(0.12) },
  border: { strong: white(0.25), subtle: white(HAIRLINE) },
  text: {
    primary: bone,
    secondary: white(0.68),
    tertiary: white(0.46),
    // A light blue fill needs dark text on it, not more light.
    onAccent: black,
    heading: white(0.68),
  },
  // The primary blue is too dark to hold a line on near black, so the stage takes it
  // at the lightness this surface needs. Same colour, read at a different depth.
  accent: { base: "#5b8bf5", hover: "#7ba4f8", wash: "rgba(91, 139, 245, 0.16)" },
  selection: bone,
  status: { danger: "#ff6b63" },
  media: { video: cobalt.base, videoEdge: cobalt.edge },
};

export const paperTheme: Theme = {
  ...inkTheme,
  id: "paper",
  name: "Paper",
  scheme: "light",

  color: {
    surface: {
      base: grey,
      gutter: grey,
      panel: blue,
      // A lift on the blue is a dark wash, not another blue: it stays right however
      // deep the blue underneath it goes.
      raised: onBlue(0.22),
      input: onBlue(0.18),
      overlay: "#1d3f9b",
      locked: onBlue(0.34),
    },
    state: { hover: white(0.1), selected: white(0.18) },
    border: { strong: white(0.3), subtle: white(HAIRLINE) },
    text: {
      primary: bone,
      secondary: white(0.74),
      tertiary: white(0.52),
      // A white button on the blue card, with the blue read back out of it.
      onAccent: blue,
      onMedia: white(0.94),
      heading: white(0.74),
    },
    // Blue on blue says nothing, so on the card the accent is white.
    accent: { base: bone, hover: "#ffffff", wash: white(0.18) },
    wordmark: { core: "#f1f0f3", edge: "#d6d5db" },
    selection: bone,
    // Video is the one media colour that shares a hue with the card, so on the blue it
    // takes the lighter step and reads as a chip rather than a smudge. The stage puts
    // the deeper one back, because there it sits on near black.
    media: {
      video: cobalt.edge,
      videoEdge: "#8fb0e4",
      audio: verdigris.base,
      audioEdge: verdigris.edge,
      image: clay.base,
      imageEdge: clay.edge,
    },
    status: { danger: "#ff6b63" },
    // The viewer is not a card. It is a window cut in the ground, so the picture sits
    // on the grey; only the frame itself is black, which is what letterboxing has to be.
    viewer: { surround: grey, matte: "#000000" },
    scrim: "rgba(14, 16, 24, 0.46)",
    shadow: "rgba(12, 16, 36, 0.2)",
  },

  surfaces: {
    chrome,
    stage,
    // A well is an inset inside a card, and every card is dark now, so a well is the
    // stage. It used to be a dark wash on the blue, which was right while panels were
    // blue and reads as a navy tint over near black. The same override also reaches
    // every overlay, so this is what stops menus, dialogs and tooltips being the last
    // blue things in the app.
    well: stage,
  },

  elevation: {
    dialog: "0 24px 48px -12px rgba(12, 16, 36, 0.3), 0 2px 8px rgba(12, 16, 36, 0.14)",
    popover: "0 12px 24px -8px rgba(12, 16, 36, 0.24), 0 1px 3px rgba(12, 16, 36, 0.12)",
  },
};
