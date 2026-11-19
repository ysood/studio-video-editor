// Studio: the product's one theme, and a dark one.
//
// A video editor is dark for a reason that is not fashion: every judgement made in it
// is a judgement about a picture, and a bright interface around a picture changes what
// you think the picture is doing. Premiere, Resolve and Avid all arrive at the same
// answer, and so does this.
//
// The chrome is shadcn/ui's stock dark surfaces at chroma zero, so there is no hue
// anywhere in the frame. Colour appears in exactly three places:
//
//   1. The clips. Blue, green and violet mean media kind, and they are vivid because
//      they are the only thing in the product a person reads meaning from colour.
//   2. The playhead, which is red, because time has to be found without looking for it
//      and red is the one hue none of the clips occupy.
//   3. Danger: offline media, destructive menu items.
//
// The primary action is white, as it is in any shadcn dark theme. Selection is white
// too, and that is not a collision: "this is now" is red, so the only two whites in
// play are "this is chosen" and "this is the action", which never appear on the same
// element.

import {
  accentWhite,
  geistFaces,
  geistMono,
  geistSans,
  mediaBlue,
  mediaGreen,
  mediaViolet,
  neutral,
  red,
} from "../primitives";
import type { Theme } from "../types";

export const studioTheme: Theme = {
  id: "studio",
  name: "Studio",
  scheme: "dark",

  color: {
    // Flat, and nearly black. The window and every panel in it are one surface, told
    // apart by hairlines, as in Premiere and Resolve.
    //
    // That surface sits a few points above black on purpose, and it is the whole of how
    // depth works here: with the ground at 925, true black is free to mean "sunk". A
    // field, a search box, a switcher's trough and the timeline's stage are all black
    // cut into the surface, rather than grey boxes standing on it. Grey then only ever
    // means raised (a quiet button, the chosen segment, a menu), and there is very
    // little of it.
    surface: {
      base: neutral[925],
      gutter: neutral[925],
      panel: neutral[925],
      raised: neutral[900],
      input: neutral[1000],
      overlay: neutral[900],
      locked: neutral[900],
    },

    // What is happening to a thing, rather than what the thing is. Translucent, so one
    // pair of values is right over a bare row and over a raised button alike and the
    // theme never has to keep two ladders of opaque greys in step.
    state: {
      hover: "rgba(255, 255, 255, 0.06)",
      selected: "rgba(255, 255, 255, 0.10)",
    },

    // White at 6% and 10%, a step under shadcn's own 10 and 15: in a window that is
    // nothing but panels the rules add up, and at shadcn's weight the grid of lines was
    // the loudest thing on screen. Alphas rather than
    // greys, so one value is correct on the panel, inside a well and on the stage, and
    // a hairline never has to be re-picked per surface.
    border: {
      strong: "rgba(255, 255, 255, 0.10)",
      subtle: "rgba(255, 255, 255, 0.06)",
    },

    text: {
      primary: neutral[50], // shadcn `foreground`
      secondary: neutral[400], // shadcn `muted-foreground`
      tertiary: neutral[500],
      onAccent: accentWhite.ink,
      onMedia: "rgba(255, 255, 255, 0.95)",
      heading: neutral[400],
    },

    accent: {
      base: accentWhite.base,
      hover: accentWhite.bright,
      wash: "rgba(255, 255, 255, 0.12)",
    },

    // Lit from inside: the core sits above the ground and the edge below it, so the
    // letters look pressed into the page rather than printed on it.
    // Both ends stay above the ground. With the edge at black on a near-black window
    // the outer half of every letter was simply not there.
    wordmark: { core: neutral[700], edge: neutral[900] },

    selection: "#ffffff",
    playhead: red.now,

    media: {
      video: mediaBlue.base,
      videoBand: mediaBlue.band,
      audio: mediaGreen.base,
      audioBand: mediaGreen.band,
      image: mediaViolet.base,
      imageBand: mediaViolet.band,
    },

    status: { danger: red.danger },

    // Neutral around the picture and black behind it. Nothing else may sit here.
    viewer: {
      surround: neutral[925],
      matte: neutral[1000],
    },

    scrim: "rgba(0, 0, 0, 0.68)",
    shadow: "rgba(0, 0, 0, 0.6)",
  },

  // Only the stage differs: the timeline canvas and the viewer's frame sit on true
  // black, so an empty track reads as part of the timeline and the picture has nothing
  // behind it. Chrome, panels and wells are deliberately the same surface.
  //
  // `well` is the surface overlays carry: dialogs, menus, popovers. They are themselves
  // raised, so everything inside one has to step again or it vanishes: a quiet button
  // at the raised grey on a dialog at the raised grey is a button with no fill, and an
  // input likewise. Inside an overlay, raised goes up one more and an input sinks back
  // to black.
  surfaces: {
    well: { surface: { panel: neutral[900], raised: neutral[800], input: neutral[1000] } },
    stage: { surface: { base: neutral[1000], panel: neutral[950] } },
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
   * The scale is four sizes and it is closed: 11, 12, 13, 20.
   *
   *   11  caption   what annotates something else: column headings, hints, the status bar
   *   12  label     the interface itself: every control, row, field, tab and timecode
   *   13  body      prose meant to be read in sentences, and the project's own name
   *   20  display   the one large readout
   *
   * Almost everything in an editor is 12. Hierarchy inside a panel comes from colour
   * (primary, secondary, tertiary) and from the two weights, never from a size picked
   * for the occasion: that is how a list ends up with a 13px name under a 12px heading
   * beside an 11px value.
   *
   * Two weights, 400 and 500, and nothing heavier. Line height is about 1.3 for anything
   * dense and 1.5 for prose.
   */
  type: {
    caption: { family: "ui", size: 11, weight: 400, lineHeight: 1.4, tracking: 0 },
    label: { family: "ui", size: 12, weight: 400, lineHeight: 1.35, tracking: 0 },
    body: { family: "ui", size: 13, weight: 400, lineHeight: 1.5, tracking: 0 },
    heading: { family: "ui", size: 12, weight: 500, lineHeight: 1.3, tracking: 0 },
    title: { family: "display", size: 13, weight: 500, lineHeight: 1.3, tracking: 0 },
    timecode: { family: "numeric", size: 12, weight: 500, lineHeight: 1.2, tracking: 0 },
    timecodeDisplay: { family: "numeric", size: 20, weight: 500, lineHeight: 1.1, tracking: -0.02 },
    // Ruler labels and clip names. A step under caption: on the canvas they are marks
    // on an instrument, read at a glance against a tick, not text to be read.
    canvas: { family: "ui", size: 10, weight: 400, lineHeight: 1, tracking: 0 },
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

  // 1.25 on a 16 unit grid, which comes out a shade under a pixel and a quarter at the
  // size icons are actually drawn. At 1.5 the set read as heavy next to Geist, which is
  // the comparison that matters: an icon is a word in the same sentence as its label.
  border: { width: 1, focusWidth: 2, iconStroke: 1.25 },

  size: {
    controlSmall: 24,
    control: 28,
    controlLarge: 36,
    // One bar height, for the top bar and for the header and footer of every panel. A
    // control is 28, so a bar leaves 8 above and below it: enough that nothing in a bar
    // sits a hair off the rule beside it, which reads as an accident.
    topbar: 44,
    statusbar: 26,
    toolbar: 44,
    tabbar: 44,
    panelFooter: 44,
    playButton: 28,
    swatch: 20,
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
    playheadHead: 5,
    playheadTop: 8,
    selectionWidth: 2,
    ghostWidth: 1,
    ghostDash: [4, 3],
    tickLength: 8,
    tickMinSpacing: 96,
    tickLabelInset: 5,
    // The band is opaque now: it is a darker shade of the clip's own hue rather than a
    // wash over it, because that is what makes the name legible on a vivid fill.
    alpha: { disabled: 0.4, lifted: 0.25, band: 1 },
  },
};
