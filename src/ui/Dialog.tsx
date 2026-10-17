import { useEffect, useRef, useState, type ReactNode } from "react";
import { OverlayContainer } from "./overlay";
import "./dialog.css";

/** Modal dialog on the native <dialog> element, which traps focus and handles Escape. */
export function Dialog({ open, onClose, title, children, className }: {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  className?: string;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const [element, setElement] = useState<HTMLDialogElement | null>(null);
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
      className={className ? `ui-dialog ${className}` : "ui-dialog"}
      onClose={onClose}
      onCancel={onClose}
      aria-label={title}
      data-surface="well"
    >
      <h2 className="ui-dialog-title">{title}</h2>
      <OverlayContainer.Provider value={element}>{children}</OverlayContainer.Provider>
    </dialog>
  );
}
