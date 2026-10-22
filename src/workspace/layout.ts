// Saved panel layout. Separate from Workspace.tsx so actions can reset the layout without
// importing Dockview.

// Bump when panels are added or renamed so stale saved layouts are discarded.
export const LAYOUT_KEY = "studio.layout.v6";

export function resetLayout() {
  try {
    localStorage.removeItem(LAYOUT_KEY);
  } catch {
    // Storage can be unavailable; reloading still restores the default layout.
  }
  window.location.reload();
}
