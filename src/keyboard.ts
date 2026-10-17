// Global shortcuts. Ignored while typing in a field so text editing keeps native keys.

import { useEffect } from "react";
import { runAction, type MenuAction } from "./actions";
import { isTextInput } from "./focus";

const IS_MAC = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

/** A shortcut label for menus and tooltips, written the way the platform writes it. */
export function chord(key: string, { shift = false } = {}): string {
  if (IS_MAC) return `${shift ? "⇧" : ""}⌘${key}`;
  return `Ctrl+${shift ? "Shift+" : ""}${key}`;
}

export type ShortcutHandler = (event: KeyboardEvent) => boolean;

const handlers = new Set<ShortcutHandler>();

/** Register a handler; return true from it to mark the key as handled. */
export function useShortcut(handler: ShortcutHandler) {
  useEffect(() => {
    handlers.add(handler);
    return () => {
      handlers.delete(handler);
    };
  }, [handler]);
}

/** App-level shortcuts. The native menu declares the same accelerators. */
function appAction(event: KeyboardEvent): MenuAction | null {
  if (!(event.metaKey || event.ctrlKey)) return null;
  switch (event.key.toLowerCase()) {
    case "z":
      return event.shiftKey ? "redo" : "undo";
    case "s":
      return event.shiftKey ? "saveAs" : "save";
    case "o":
      return "openProject";
    case "n":
      return "newProject";
    case "i":
      return "importMedia";
    case "e":
      return "export";
    default:
      return null;
  }
}

export function useGlobalShortcuts() {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (isTextInput(event.target)) return;
      const action = appAction(event);
      let handled = action !== null;
      if (action) runAction(action, "keyboard");
      else {
        for (const handler of handlers) {
          if (handler(event)) {
            handled = true;
            break;
          }
        }
      }
      if (handled) event.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
