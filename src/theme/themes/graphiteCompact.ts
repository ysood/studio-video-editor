// A denser variant of Graphite. It exists to prove that density is one central change:
// every size below flows through the whole UI, including the timeline canvas.

import type { Theme } from "../types";
import { graphiteTheme as base } from "./graphite";

export const graphiteCompactTheme: Theme = {
  ...base,
  id: "graphite-compact",
  name: "Graphite compact",
  size: {
    ...base.size,
    controlSmall: 20,
    control: 22,
    field: 22,
    topbar: 34,
    statusbar: 22,
    toolbar: 28,
    tabbar: 24,
    transport: 44,
    playButton: 28,
    swatch: 22,
  },
  timeline: {
    ...base.timeline,
    rulerHeight: 22,
    rowHeight: 34,
    headerWidth: 128,
    clipInset: 2,
    clipBand: 13,
  },
};
