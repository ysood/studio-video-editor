import { createContext, useContext } from "react";

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
