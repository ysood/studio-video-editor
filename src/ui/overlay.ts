import { createContext, useContext, useEffect, useState, useSyncExternalStore } from "react";

/**
 * Where overlays (menus, select lists, popovers, tooltips) portal to. Normally the
 * document body; inside a modal <dialog> it must be the dialog itself, because the
 * dialog sits in the browser's top layer and anything portalled to body would render
 * underneath it.
 */
export const OverlayContainer = createContext<HTMLElement | null>(null);

export function useOverlayContainer(): HTMLElement | undefined {
  return useContext(OverlayContainer) ?? undefined;
}

/**
 * How many overlays are open anywhere in the app.
 *
 * The native preview surface (ADR-0003) is a real view sitting above the webview, so it
 * paints over whatever the web layer draws in the same place: a dialog over the viewer
 * comes out half buried. The viewer watches this so it can get out of the way while
 * something is open. Nothing else needs it, and the count is kept here rather than in the
 * app's store so the primitives do not have to know the app exists.
 */
let openCount = 0;
const listeners = new Set<() => void>();

/**
 * Track an overlay's open state for the duration of this component.
 *
 * Returns the state and its setter, for a Radix root to be controlled by. Counting in an
 * effect rather than in the setter means an overlay that unmounts while open still counts
 * down, which a stray open would otherwise leave stuck.
 */
export function useOverlayOpen(): [boolean, (open: boolean) => void] {
  const [open, setOpen] = useState(false);
  useEffect(() => {
    if (!open) return;
    openCount += 1;
    listeners.forEach((listen) => listen());
    return () => {
      openCount = Math.max(0, openCount - 1);
      listeners.forEach((listen) => listen());
    };
  }, [open]);
  return [open, setOpen];
}

/** Whether anything is open over the page right now. */
export function useOverlaysOpen(): boolean {
  return useSyncExternalStore(
    (listen) => {
      listeners.add(listen);
      return () => void listeners.delete(listen);
    },
    () => openCount > 0,
    () => false,
  );
}
