// Graphite: the baseline theme. Neutral chrome so it never tints picture judgement;
// color carries meaning only (video, audio, and amber for time).

import { amber, coral, graphite, sea, steel, systemSans } from "../primitives";
import type { Theme } from "../types";

export const graphiteTheme: Theme = {
  id: "graphite",
  name: "Graphite",
  scheme: "dark",

  color: {
    surface: {
      base: graphite[950],
      gutter: graphite[950],
      panel: graphite[900],
      raised: graphite[850],
      hover: graphite[750],
      input: graphite[950],
      overlay: graphite[900],
      locked: graphite[850],
    },
    border: {
      strong: graphite[700],
      subtle: graphite[800],
    },
    text: {
      primary: graphite[50],
      secondary: graphite[300],
      tertiary: graphite[500],
      onAccent: amber[950],
      onMedia: "rgba(255, 255, 255, 0.92)",
      heading: graphite[300],
    },
    accent: {
      base: amber[500],
      hover: amber[400],
      wash: "rgba(240, 181, 69, 0.12)",
    },
    // Graphite is the plain theme: no light falls on its wordmark.
    wordmark: { core: "#0a0a0b", edge: "#0a0a0b" },
    selection: amber[500],
    media: {
      video: steel[500],
      videoEdge: steel[300],
      audio: sea[500],
      audioEdge: sea[300],
      image: "color-mix(in srgb, #4d79a8 55%, #303237)",
      imageEdge: steel[300],
    },
    status: {
      danger: coral[500],
    },
    viewer: {
      surround: graphite[950],
      matte: graphite[1000],
    },
    scrim: "rgba(10, 10, 12, 0.6)",
    shadow: "rgba(0, 0, 0, 0.5)",
  },

  surfaces: {},

  font: {
    ui: systemSans,
    display: systemSans,
    numeric: systemSans,
    // Graphite is the plain theme: its wordmark is the same sans as everything else.
    wordmark: systemSans,
    faces: [],
    numericVariant: "tabular-nums",
  },

  type: {
    caption: { family: "ui", size: 11, weight: 400, lineHeight: 1.4, tracking: 0 },
    // label and body share a size here on purpose: in a dense editor they do the same
    // typographic job, and they are told apart by color at the point of use. They stay
    // separate roles so a denser theme can pull them apart.
    label: { family: "ui", size: 13, weight: 400, lineHeight: 1.4, tracking: 0 },
    body: { family: "ui", size: 13, weight: 400, lineHeight: 1.4, tracking: 0 },
    title: { family: "display", size: 17, weight: 600, lineHeight: 1.25, tracking: 0 },
    heading: { family: "ui", size: 13, weight: 600, lineHeight: 1.4, tracking: 0 },
    timecode: { family: "numeric", size: 15, weight: 600, lineHeight: 1.2, tracking: 0.02 },
    timecodeDisplay: { family: "numeric", size: 22, weight: 600, lineHeight: 1.1, tracking: 0.02 },
    canvas: { family: "ui", size: 11, weight: 400, lineHeight: 1, tracking: 0 },
    // Large text needs no extra leading, and capitals need tracking to stay readable.
    wordmark: { family: "wordmark", size: 60, weight: 400, lineHeight: 1, tracking: 0.22 },
    // Capitals this small are unreadable without air between them.
    overline: { family: "ui", size: 11, weight: 500, lineHeight: 1.2, tracking: 0.38 },
    projectName: { family: "wordmark", size: 20, weight: 500, lineHeight: 1.2, tracking: 0 },
  },

  // Steps 1 to 9 are the editor's own, close together because panels are dense. The
  // last three are for the places that are not an editor, where the scale has to open
  // out: each is a third again on the one before, so no two are confusable.
  space: { hairline: 1, 1: 2, 2: 4, 3: 6, 4: 8, 5: 10, 6: 12, 7: 16, 8: 20, 9: 24, 10: 32, 11: 48, 12: 64 },

  radius: { control: 3, panel: 0, dialog: 6, clip: 3, round: 9999 },

  border: { width: 1, focusWidth: 2, iconStroke: 1.4 },

  size: {
    controlSmall: 22,
    control: 26,
    controlLarge: 36,
    topbar: 40,
    statusbar: 24,
    toolbar: 32,
    tabbar: 28,
    panelFooter: 40,
    playButton: 34,
    swatch: 28,
    icon: 14,
    dot: 7,
    progressWidth: 120,
    progressHeight: 4,
    dialogWidth: 520,
    fieldLabel: 124,
    zoomSlider: 120,
    timecodeMin: 96,
    viewerPadding: 16,
    binPane: 136,
    thumbnail: 128,
    thumbnailMin: 96,
    thumbnailMax: 288,
    menuMinWidth: 184,
    homeColumn: 640,
    wordmarkCap: 220,
    sliderTrack: 4,
    sliderThumb: 12,
  },

  opacity: { disabled: 0.4 },

  motion: { quick: 120, base: 200, easing: "cubic-bezier(0.2, 0, 0, 1)", tooltipDelay: 450 },

  elevation: {
    dialog: "0 24px 60px rgba(0, 0, 0, 0.5)",
    popover: "0 8px 24px rgba(0, 0, 0, 0.45)",
  },

  timeline: {
    rulerHeight: 26,
    rowHeight: 46,
    rowGap: 1,
    headerWidth: 148,
    headerStripe: 3,
    edgeGrab: 6,
    snap: 8,
    clipInset: 3,
    clipGap: 1,
    clipBand: 15,
    clipLabelInset: 5,
    offlineStripe: 2,
    playheadWidth: 1,
    playheadHead: 6,
    selectionWidth: 2,
    ghostWidth: 1,
    ghostDash: [4, 3],
    tickLength: 8,
    tickMinSpacing: 90,
    tickLabelInset: 4,
    alpha: { disabled: 0.4, lifted: 0.25, band: 0.35 },
  },
};
