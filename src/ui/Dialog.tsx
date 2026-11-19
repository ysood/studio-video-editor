import { useEffect, useRef, useState, type ReactNode } from "react";
import { OverlayContainer, useOverlayOpen } from "./overlay";
import { cn } from "../lib/cn";

/**
 * No z-index and no portal: a modal <dialog> is in the browser's top layer, above every
 * z-index on the page, and its backdrop with it. Width is the theme's, and the user-agent
 * `max-width` on <dialog> is what keeps it inside a narrow window, which is why it is not
 * restated here.
 */
const dialogPanel =
  "w-dialog-width rounded-dialog border border-edge-strong bg-overlay p-8 text-fg shadow-dialog backdrop:bg-scrim";

/** Modal dialog on the native <dialog> element, which traps focus and handles Escape. */
export function Dialog({ open, onClose, title, children, className }: {
  open: boolean;
  onClose: () => void;
  title: string;
  /** Optional so a dialog can render closed before it has anything to show. */
  children?: ReactNode;
  className?: string;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const [element, setElement] = useState<HTMLDialogElement | null>(null);
  const [, setCounted] = useOverlayOpen();
  useEffect(() => setCounted(open), [open, setCounted]);
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);
  return (
    <dialog
      ref={(node) => {
        ref.current = node;
        setElement(node);
      }}
      className={cn(dialogPanel, className)}
      onClose={onClose}
      onCancel={onClose}
      aria-label={title}
      data-surface="well"
    >
      <h2 className="mb-6 text-title text-fg">{title}</h2>
      <OverlayContainer.Provider value={element}>{children}</OverlayContainer.Provider>
    </dialog>
  );
}
