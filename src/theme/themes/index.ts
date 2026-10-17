import type { Theme } from "../types";
import { graphiteTheme } from "./graphite";
import { graphiteCompactTheme } from "./graphiteCompact";

/** Every theme the app can switch to. Add a theme by adding a file and listing it here. */
export const themes: Theme[] = [graphiteTheme, graphiteCompactTheme];

export const defaultTheme = graphiteTheme;
