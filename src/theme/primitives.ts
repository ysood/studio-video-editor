// Raw values. Themes pick from these; components never import this file.

import geistMonoVariable from "./fonts/GeistMono-Variable.woff2";
import geistVariable from "./fonts/Geist-Variable.woff2";

/**
 * Neutral: the one grey ramp, and the whole of Studio's chrome.
 *
 * These are shadcn/ui's stock dark surfaces, which are Tailwind's `neutral` scale, and
 * the important thing about them is that their chroma is exactly zero. An earlier ramp
 * here carried about 8% of blue on the theory that a trace of hue reads as an
 * instrument. It does not. At these lightnesses a tinted grey reads as a grey that has
 * gone wrong, the whole window looks lifted toward blue, and on a tool whose job is
 * judging colour that is the one mistake worth never making. Pure neutral it is.
 *
 * The named steps map onto shadcn's semantic tokens:
 *
 *   950  background        the window
 *   900  card              a panel
 *   800  secondary, muted  anything raised, and popovers
 *   500  ring              tertiary text
 *   400  muted-foreground  secondary text
 *   200  primary           the accent, which in a shadcn dark theme is near white
 *   50   foreground        primary text
 */
export const neutral = {
  50: "#fafafa",
  100: "#f5f5f5",
  200: "#e5e5e5",
  300: "#d4d4d4",
  400: "#a1a1a1",
  500: "#737373",
  600: "#525252",
  700: "#404040",
  800: "#262626",
  900: "#171717",
  925: "#101010", // the window and its panels: just off black, so black can mean "sunk"
  950: "#0a0a0a",
  1000: "#000000",
} as const;

/**
 * The accent is white, because in a shadcn dark theme the primary action is a near-white
 * fill with dark text on it, and because a video editor should not be sprinkled with a
 * brand hue. Colour in this product belongs to the clips.
 */
export const accentWhite = {
  base: neutral[200],
  bright: neutral[50], // hover
  ink: neutral[900], // text on a white fill
} as const;

/**
 * Red does two jobs, and they never share a surface.
 *
 * `now` is the playhead. With the accent neutral, time needs a colour of its own, and a
 * red position indicator is what Resolve uses: against blue, green and violet clips it
 * is the one hue that cannot be mistaken for any of them, and a hairline has to be found
 * instantly. `danger` is the softer red for text and icons, taken from shadcn's own dark
 * `destructive`, because an alarm set in type wants less saturation than a one pixel line.
 */
export const red = { now: "#ff4538", danger: "#ff6467" } as const;

/**
 * Media hues. These are the only saturated fills in Studio, and the only place a person
 * is meant to read meaning from colour, so they are the one thing allowed to be vivid.
 *
 * The assignment follows Premiere and Resolve rather than taste: video is blue
 * (Premiere's Iris), audio is green (Caribbean, and Resolve's audio green), stills are
 * violet (Lavender). An editor arriving from either already knows what a green bar is.
 *
 * Each kind is a pair. `base` fills the clip and is as saturated as it can be. `band` is
 * the same hue taken down hard, and it is the strip along the top of the clip that
 * carries the name. That is why the band darkens rather than lightens, which is the
 * opposite of what it used to do: white type on a vivid fill sits at about 3:1, which is
 * not enough at 11px, and the same type on the band clears 7:1. Premiere draws its clip
 * names on a darker header for the same reason.
 */
export const mediaBlue = { base: "#2f6bff", band: "#1b3da8" } as const;
export const mediaGreen = { base: "#0fa97f", band: "#075f48" } as const;
export const mediaViolet = { base: "#a855f7", band: "#5f2b91" } as const;

export const geistSans = '"Geist", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif';
export const geistMono = '"Geist Mono", ui-monospace, "SF Mono", Menlo, monospace';

export const geistFaces = [
  { family: "Geist", src: geistVariable, weight: "100 900" },
  { family: "Geist Mono", src: geistMonoVariable, weight: "100 900" },
] as const;
