import { useEffect, useRef, type ReactNode } from "react";
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
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);
  return (
    <dialog
      ref={ref}
      className={className ? `ui-dialog ${className}` : "ui-dialog"}
      onClose={onClose}
      onCancel={onClose}
      aria-label={title}
      data-surface="well"
    >
      <h2 className="ui-dialog-title">{title}</h2>
      {children}
    </dialog>
  );
}
