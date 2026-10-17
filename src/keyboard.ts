// Global shortcuts. Ignored while typing in a field so text editing keeps native keys.

import { useEffect } from "react";
import { importMedia, newProject, openProject, saveProject } from "./actions";

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

function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
}

export function useGlobalShortcuts(actions: { undo: () => void; redo: () => void }) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (isTyping(event.target)) return;
      const mod = event.metaKey || event.ctrlKey;
      const key = event.key.toLowerCase();
      let handled = true;
      if (mod && key === "z" && event.shiftKey) actions.redo();
      else if (mod && key === "z") actions.undo();
      else if (mod && key === "s") void saveProject(event.shiftKey);
      else if (mod && key === "o") void openProject();
      else if (mod && key === "n") void newProject();
      else if (mod && key === "i") void importMedia();
      else {
        handled = false;
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
  }, [actions]);
}
