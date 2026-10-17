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

/**
 * Where a piece of UI sits. Surfaces let a theme color regions differently, for example a
 * saturated chrome frame around dark content wells.
 * - chrome: top bar, status bar, panel tab strips, gaps between panels
 * - panel: panel bodies (the default)
 * - well: inset content inside a panel (bin list, inspector groups, dialogs)
 * - stage: picture and time (viewer frame, timeline)
 */
export type SurfaceName = "chrome" | "panel" | "well" | "stage";

export type DeepPartial<T> = { [K in keyof T]?: T[K] extends object ? DeepPartial<T[K]> : T[K] };

export interface ColorRoles {
  surface: {
    base: string; // app background behind everything; on chrome, the panel tab strips
    gutter: string; // the gaps between dock panels
    panel: string; // the background of the current surface
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
    heading: string; // section headings inside panels
  };
  accent: {
    base: string; // time and action: playhead, timecode, primary button, focus
    hover: string;
    wash: string; // translucent accent for drop targets
  };
  /** Outline of selected clips and bin items. */
  selection: string;
  media: {
    video: string;
    videoEdge: string;
    audio: string;
    audioEdge: string;
    image: string;
    imageEdge: string;
  };
  status: {
    danger: string;
  };
  viewer: {
    surround: string; // around the picture frame
    matte: string; // behind picture inside the frame
  };
  scrim: string; // behind modal dialogs
  shadow: string; // shadow color for elevation
}

export interface Theme {
  id: string;
  name: string;
  scheme: "dark" | "light";

  /** Base colors. They apply to the "panel" surface and to anything not inside another. */
  color: ColorRoles;

  /**
   * Per-surface color overrides. Any element with `data-surface="<name>"` gets the base
   * colors merged with that surface's overrides, so the same component reads correctly on
   * a light chrome bar and inside a dark well. Leave empty for a uniform theme.
   */
  surfaces: Partial<Record<SurfaceName, DeepPartial<ColorRoles>>>;

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
    binPane: number; // width of the media bin's folder column
    thumbnail: number; // default gallery tile width
    thumbnailMin: number;
    thumbnailMax: number;
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
