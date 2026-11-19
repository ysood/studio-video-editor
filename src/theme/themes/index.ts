import type { Theme } from "../types";
import { studioTheme } from "./studio";

/**
 * Every theme the app can switch to.
 *
 * Studio ships one, and it is dark: an interface around a picture must not change what
 * you think the picture is doing. The registry stays because the canvas painters and
 * the design gallery both read a theme as an object rather than as the document's
 * current variables, and because a second theme is a file plus a line here. It is not
 * an invitation to add one; a look that needs a new theme should usually be a change
 * to this one.
 */
export const themes: Theme[] = [studioTheme];

export const defaultTheme = studioTheme;
