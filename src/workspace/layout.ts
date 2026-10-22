// Saved panel layout, and the handful of layout changes that start from somewhere else
// in the app. Separate from Workspace.tsx so actions can reach the dock without
// importing Dockview; only its type is imported here, which compiles away.

import type { DockviewApi } from "dockview-react";

// Bump when the default layout changes in a way a saved one cannot be read as.
export const LAYOUT_KEY = "studio.layout.v6";

/** The panels the default layout places. A saved layout missing one of these is stale. */
export const DEFAULT_PANELS = ["bin", "viewer", "inspector", "timeline"] as const;

/** The chat panel floats rather than taking a share of the grid (see `toggleChat`). */
const CHAT = { id: "chat", width: 380, height: 560, inset: 24 };

let dock: DockviewApi | null = null;

/** Handed the dock once it is ready, so actions elsewhere can reach it. */
export function holdDock(api: DockviewApi) {
  dock = api;
}

/**
 * Show the assistant, or put it away again.
 *
 * Floating rather than docked: the grid was arranged so the viewer sits in the middle of
 * the window, and a fifth column would push it off centre every time somebody wanted to
 * ask a question. Floating, it can also be dragged out to a window of its own, which is
 * what a long conversation wants.
 */
export function toggleChat() {
  if (!dock) return;
  const existing = dock.getPanel(CHAT.id);
  if (existing) {
    existing.api.close();
    return;
  }
  dock.addPanel({
    id: CHAT.id,
    component: CHAT.id,
    title: "Assistant",
    floating: {
      width: CHAT.width,
      height: CHAT.height,
      // Near the right edge, clear of the window's own chrome, and clamped so a narrow
      // window does not put it off screen.
      x: Math.max(CHAT.inset, dock.width - CHAT.width - CHAT.inset),
      y: CHAT.inset,
      // Dragged by its own tab strip. The default adds a second bar above the tab for
      // the purpose, and two rows of chrome on a panel is one more than anything else
      // in the app has.
      dragHandle: "tabbar",
    },
  });
}

export function resetLayout() {
  try {
    localStorage.removeItem(LAYOUT_KEY);
  } catch {
    // Storage can be unavailable; reloading still restores the default layout.
  }
  window.location.reload();
}
