import type { Theme } from "../types";
import { crimsonTheme } from "./crimson";
import { graphiteTheme } from "./graphite";
import { graphiteCompactTheme } from "./graphiteCompact";
import { inkTheme } from "./ink";
import { kitTheme } from "./kit";
import { paperTheme } from "./paper";

/** Every theme the app can switch to. Add a theme by adding a file and listing it here. */
export const themes: Theme[] = [
  inkTheme,
  paperTheme,
  kitTheme,
  crimsonTheme,
  graphiteTheme,
  graphiteCompactTheme,
];

export const defaultTheme = paperTheme;
