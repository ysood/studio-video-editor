// Studio: the product's one theme, and a dark one.
//
// A video editor is dark for a reason that is not fashion: every judgement made in it
// is a judgement about a picture, and a bright interface around a picture changes what
// you think the picture is doing. Premiere, Resolve and Avid all arrive at the same
// answer, and so does this.
//
// The whole idea of the palette is that colour is rationed. The chrome is one neutral
// ramp and nothing else; hue appears in exactly three places:
//
//   1. Amber means time and the action that commits it: the playhead, the timecode,
//      the one primary button in a region. Nothing else.
//   2. Blue, green and violet mean media kind, on clips, bin swatches and tiles.
//      These are the only saturated fills in the product.
//   3. Signal means danger: offline media, destructive menu items.
//
// Selection is deliberately none of them. It is plain white, because if selection were
// the accent then "this is chosen" and "this is now" would be the same colour on the
// same timeline.

import { amber, ash, caribbean, geistFaces, geistMono, geistSans, iris, lavender, signal } from "../primitives";
import type { Theme } from "../types";

export const studioTheme: Theme = {
  id: "studio",
  name: "Studio",
  scheme: "dark",

  color: {
    surface: {
      base: ash[950],
      // The gutter is darker than the window, not lighter: a card reads as a card
      // because you can see a shadow of ground around it, and here the ground is a gap.
      gutter: ash[975],
      panel: ash[900],
      raised: ash[800],
      input: ash[950],
      // Overlays are the one surface that rises. A menu has to read as lifted off
      // whatever it covers, and a step that only just clears the panel reads as a
      // rendering artefact rather than as a thing floating over it.
      overlay: ash[800],
      locked: ash[850],
    },

    // What is happening to a thing, rather than what the thing is. Translucent, so one
    // pair of values is right over a bare row and over a raised button alike and the
    // theme never has to keep two ladders of opaque greys in step.
    state: {
      hover: "rgba(255, 255, 255, 0.055)",
      selected: "rgba(255, 255, 255, 0.10)",
    },

    // Borders are the text they separate, faded, which is why they are alphas and not
    // greys: one value is correct on the panel, inside a well and on the stage, and a
    // hairline never has to be re-picked per surface.
    border: {
      strong: "rgba(255, 255, 255, 0.13)",
      subtle: "rgba(255, 255, 255, 0.07)",
    },

    text: {
      primary: ash[50],
      secondary: ash[400],
      tertiary: ash[500],
      onAccent: amber.ink,
      onMedia: "rgba(255, 255, 255, 0.95)",
      heading: ash[400],
    },

    accent: {
      base: amber.base,
      hover: amber.bright,
      wash: "rgba(240, 162, 60, 0.14)",
    },

    // Lit from inside: the core sits above the ground and the edge below it, so the
    // letters look pressed into the page rather than printed on it.
    wordmark: { core: ash[800], edge: ash[1000] },

    // Not the accent. See the note at the top of the file.
    selection: "#ffffff",

    media: {
      video: iris.base,
      videoEdge: iris.edge,
      audio: caribbean.base,
      audioEdge: caribbean.edge,
      image: lavender.base,
      imageEdge: lavender.edge,
    },

    status: { danger: signal },

    // Neutral around the picture and black behind it. Nothing else may sit here.
    viewer: {
      surround: ash[975],
      matte: ash[1000],
    },

    scrim: "rgba(0, 0, 0, 0.68)",
    shadow: "rgba(0, 0, 0, 0.6)",
  },

  // Wells sink below the panel they sit in; the stage sinks further still, so an empty
  // track reads as part of the timeline rather than as the gap around it. Everything
  // else is told apart by the same hairline, not by a third fill.
  surfaces: {
    chrome: { surface: { panel: ash[950] } },
    well: { surface: { panel: ash[950], raised: ash[850], input: ash[975] } },
    stage: {
      surface: { base: ash[1000], panel: ash[975], raised: ash[850], locked: ash[800] },
    },
  },

  font: {
    ui: geistSans,
    display: geistSans,
    numeric: geistMono,
    wordmark: geistSans,
    faces: [...geistFaces],
    numericVariant: "tabular-nums",
  },

  /**
   * Three rules, and no fourth:
   *
   * 1. Two weights, 400 and 500. Nothing heavier exists. Weight tells a label from its
   *    value; it is not for emphasis. Everything that was at 600 is what made the app
   *    look patchy, because six regions each picked their own idea of important.
   * 2. Tracking goes negative as size goes up, about -0.02em from 15px, and stays at
   *    zero below it. A face spaced for 12px falls apart set at 20.
   * 3. Three line heights: 1 for display, ~1.3 for anything dense, 1.5 for prose meant
   *    to be read in sentences.
   *
   * The one exception is `overline`, small capitals, which need air between them.
   */
  type: {
    caption: { family: "ui", size: 11, weight: 400, lineHeight: 1.4, tracking: 0 },
    label: { family: "ui", size: 12, weight: 400, lineHeight: 1.35, tracking: 0 },
    body: { family: "ui", size: 13, weight: 400, lineHeight: 1.5, tracking: 0 },
    heading: { family: "ui", size: 12, weight: 500, lineHeight: 1.3, tracking: 0 },
    title: { family: "display", size: 15, weight: 500, lineHeight: 1.2, tracking: -0.012 },
    timecode: { family: "numeric", size: 13, weight: 500, lineHeight: 1.2, tracking: 0 },
    timecodeDisplay: { family: "numeric", size: 20, weight: 500, lineHeight: 1.1, tracking: -0.02 },
    canvas: { family: "ui", size: 11, weight: 400, lineHeight: 1, tracking: 0 },
    overline: { family: "ui", size: 10, weight: 500, lineHeight: 1.2, tracking: 0.14 },
    projectName: { family: "wordmark", size: 14, weight: 500, lineHeight: 1.2, tracking: -0.005 },
    wordmark: { family: "wordmark", size: 60, weight: 500, lineHeight: 1, tracking: -0.03 },
  },

  // Steps 1 to 9 are the editor's own, close together because panels are dense. The
  // last three are for the places that are not an editor, where the scale has to open
  // out: each is a third again on the one before, so no two are confusable.
  space: { hairline: 1, 1: 2, 2: 4, 3: 6, 4: 8, 5: 10, 6: 12, 7: 16, 8: 20, 9: 24, 10: 32, 11: 48, 12: 64 },

  // Square enough to read as an instrument, round enough not to read as 2012. A pill
  // button belongs on a marketing page; a tool with forty controls in view needs them
  // to tile.
  radius: { control: 6, button: 6, panel: 8, dialog: 10, clip: 4, round: 9999 },

  border: { width: 1, focusWidth: 2, iconStroke: 1.5 },

  size: {
    controlSmall: 24,
    control: 28,
    controlLarge: 36,
    topbar: 44,
    statusbar: 26,
    toolbar: 36,
    tabbar: 30,
    panelFooter: 40,
    playButton: 34,
    swatch: 28,
    icon: 16,
    dot: 6,
    progressWidth: 120,
    progressHeight: 4,
    dialogWidth: 520,
    fieldLabel: 124,
    fieldNumber: 60,
    zoomSlider: 120,
    timecodeMin: 96,
    viewerPadding: 16,
    viewerHandle: 9,
    binPane: 168,
    thumbnail: 128,
    thumbnailMin: 96,
    thumbnailMax: 288,
    menuMinWidth: 200,
    homeColumn: 640,
    wordmarkCap: 220,
    sliderTrack: 4,
    sliderThumb: 14,
  },

  opacity: { disabled: 0.4 },

  motion: { quick: 120, base: 200, easing: "cubic-bezier(0.2, 0, 0, 1)", tooltipDelay: 450 },

  elevation: {
    dialog: "0 24px 64px rgba(0, 0, 0, 0.6)",
    popover: "0 8px 24px rgba(0, 0, 0, 0.5)",
  },

  timeline: {
    rulerHeight: 28,
    // Tall enough for a name and a waveform, which is what the row is for. Resolve and
    // Premiere both default near here and both let it grow.
    rowHeight: 52,
    rowGap: 1,
    headerWidth: 160,
    headerStripe: 3,
    edgeGrab: 6,
    snap: 8,
    clipInset: 2,
    clipGap: 1,
    clipBand: 16,
    clipLabelInset: 6,
    offlineStripe: 2,
    playheadWidth: 1,
    playheadHead: 6,
    selectionWidth: 2,
    ghostWidth: 1,
    ghostDash: [4, 3],
    tickLength: 8,
    tickMinSpacing: 96,
    tickLabelInset: 5,
    alpha: { disabled: 0.4, lifted: 0.25, band: 0.35 },
  },
};
