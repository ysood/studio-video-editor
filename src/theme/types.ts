// The shape of a Studio theme. Components and CSS only ever reference these semantic
// roles (as CSS variables or through `useTheme`), never raw values. A new look is a new
// object of this type; nothing else in the app needs to change.

export type FontRole = "ui" | "display" | "numeric";

export interface FontFaceSpec {
  /** Family name as used in the `font` stacks below. */
  family: string;
  /** URL of the font file, usually imported from `src/theme/fonts/` so Vite bundles it. */
  src: string;
  weight?: string;
  style?: "normal" | "italic";
}

export interface TypeRole {
  family: FontRole;
  /** Pixels. */
  size: number;
  weight: number;
  /** Unitless multiple of size. */
  lineHeight: number;
  /** Letter spacing in em. */
  tracking: number;
}

export type TypeRoleName =
  | "caption" // small metadata: durations, formats, status bar
  | "label" // control labels, toolbar buttons, inspector rows
  | "body" // default text
  | "title" // project title, dialog titles
  | "heading" // section headings inside panels
  | "timecode" // timeline toolbar timecode
  | "timecodeDisplay" // the viewer's large timecode
  | "canvas"; // text painted on the timeline canvas

export interface Theme {
  id: string;
  name: string;
  scheme: "dark" | "light";

  color: {
    surface: {
      base: string; // app background, viewer surround, timeline track area
      panel: string; // panel bodies
      raised: string; // buttons, track headers, selected rows
      hover: string; // hover state of raised things
      input: string; // text fields
      overlay: string; // dialogs
      locked: string; // locked track rows
    };
    border: {
      strong: string; // control outlines
      subtle: string; // dividers between regions
    };
    text: {
      primary: string;
      secondary: string;
      tertiary: string;
      onAccent: string; // text on the accent fill
      onMedia: string; // labels on clips and media swatches
    };
    accent: {
      base: string; // time: playhead, timecode, selection, primary action, focus
      hover: string;
      wash: string; // translucent accent for drop targets
    };
    media: {
      video: string;
      videoEdge: string;
      audio: string;
      audioEdge: string;
      image: string;
    };
    status: {
      danger: string;
    };
    viewer: {
      matte: string; // behind picture inside the frame
    };
    scrim: string; // behind modal dialogs
    shadow: string; // shadow color for elevation
  };

  font: Record<FontRole, string> & {
    faces: FontFaceSpec[];
    /** font-variant-numeric for numbers that must not jitter (timecode, durations). */
    numericVariant: string;
  };

  type: Record<TypeRoleName, TypeRole>;

  /** Spacing scale in pixels. Layout uses these steps only. */
  space: {
    hairline: number;
    1: number;
    2: number;
    3: number;
    4: number;
    5: number;
    6: number;
    7: number;
    8: number;
    9: number;
  };

  radius: {
    control: number;
    panel: number;
    dialog: number;
    clip: number;
    round: number;
  };

  border: {
    width: number;
    focusWidth: number;
    /** Stroke weight of line icons, in icon grid units (16 per icon). */
    iconStroke: number;
  };

  /** Fixed component dimensions in pixels. */
  size: {
    controlSmall: number; // icon buttons
    control: number; // buttons
    field: number; // text inputs and selects
    topbar: number;
    statusbar: number;
    toolbar: number;
    tabbar: number;
    transport: number;
    playButton: number;
    swatch: number;
    icon: number;
    dot: number;
    progressWidth: number;
    progressHeight: number;
    dialogWidth: number;
    fieldLabel: number;
    zoomSlider: number;
    timecodeMin: number;
    viewerPadding: number;
  };

  motion: {
    /** Milliseconds. */
    quick: number;
    base: number;
    easing: string;
  };

  elevation: {
    dialog: string;
  };

  timeline: {
    rulerHeight: number;
    rowHeight: number;
    rowGap: number;
    headerWidth: number;
    headerStripe: number; // colored edge on track headers
    /** Pixels from a clip edge that grab the edge for trimming. */
    edgeGrab: number;
    /** Snap distance in pixels. */
    snap: number;
    clipInset: number; // vertical inset of a clip inside its row
    clipGap: number; // air between butted clips
    clipBand: number; // height of the lighter name band
    clipLabelInset: number;
    offlineStripe: number;
    playheadWidth: number;
    playheadHead: number; // half-width of the playhead marker
    selectionWidth: number;
    ghostWidth: number;
    ghostDash: [number, number];
    tickLength: number;
    tickMinSpacing: number;
    tickLabelInset: number;
    alpha: {
      disabled: number;
      lifted: number;
      band: number;
    };
  };
}
