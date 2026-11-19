// Raw values. Themes pick from these; components never import this file.

import geistMonoVariable from "./fonts/GeistMono-Variable.woff2";
import geistVariable from "./fonts/Geist-Variable.woff2";

/**
 * Ash: the one neutral ramp, and the whole of Studio's chrome.
 *
 * It is a near-neutral grey carrying about 8% of blue (hue 220). A dead grey reads as
 * unfinished and a warm grey reads as paper, and neither is what a room full of people
 * grading a picture wants behind it; a trace of blue reads as an instrument. The
 * saturation stays low enough that it never shifts a judgement about what is on screen.
 *
 * The steps are even in lightness rather than in hex, which is what makes a surface
 * ladder look deliberate: panel to raised is the same visual distance as window to
 * panel, so no two surfaces look like a mistake and none look like the same one twice.
 */
export const ash = {
  50: "#f1f3f6", // primary text; not white, which buzzes on near black
  100: "#dfe3e9",
  200: "#c3c9d3",
  300: "#a1a9b6",
  400: "#949cab", // secondary text
  500: "#6b7382", // tertiary text
  600: "#4b525e",
  700: "#2e333c",
  750: "#262a32",
  800: "#1e2128", // raised
  850: "#181b21", // overlay, locked, wells lifted
  900: "#13161b", // panel
  950: "#0d0f13", // the window
  975: "#08090b", // the gutter between cards, and the stage
  1000: "#000000", // behind the picture, and nowhere else
} as const;

/**
 * Amber: the one accent, and it means one thing, time and the action that commits it.
 *
 * It is the only hue in the product that is not media. That is deliberate: a playhead
 * has to be found instantly against a timeline of blue, green and violet clips, and
 * amber is the one direction none of them occupy. It is also why the accent is not the
 * blue every editor reaches for first: the video clips already own blue.
 */
export const amber = {
  base: "#f0a23c",
  bright: "#ffbc67", // hover
  ink: "#1b1204", // text on an amber fill
} as const;

/**
 * Media hues. These are the only saturated fills in Studio, and the only place a
 * person is meant to read meaning from colour.
 *
 * The assignment follows Premiere and Resolve rather than taste: video is a blue
 * violet (Premiere's Iris), audio is a teal green (Caribbean, and Resolve's audio
 * green), stills are a violet magenta (Lavender). Editors arriving from either
 * application already know what a green bar is.
 *
 * Each kind is a pair: `base` fills the clip and `edge` draws its rim and name band,
 * one step lighter and more saturated so a clip has an edge without a second colour.
 * The three bases sit at the same lightness, so no track type shouts louder than
 * another, and far enough apart in hue to stay separable for the most common colour
 * vision deficiencies.
 */
export const iris = { base: "#3c5ccc", edge: "#7b93f2" } as const;
export const caribbean = { base: "#1a8f74", edge: "#4fc3a4" } as const;
export const lavender = { base: "#9b4fc4", edge: "#c98ae6" } as const;

/** Danger: offline media, destructive menu items. Nothing else. */
export const signal = "#f26d64";

export const geistSans = '"Geist", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif';
export const geistMono = '"Geist Mono", ui-monospace, "SF Mono", Menlo, monospace';

export const geistFaces = [
  { family: "Geist", src: geistVariable, weight: "100 900" },
  { family: "Geist Mono", src: geistMonoVariable, weight: "100 900" },
] as const;
