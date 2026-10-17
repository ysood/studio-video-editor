import * as PopoverPrimitive from "@radix-ui/react-popover";
import type { ReactNode } from "react";
import { useOverlayContainer, useOverlayOpen } from "./overlay";
import "./overlay.css";

/** A floating panel anchored to `trigger`, for small settings that do not need a dialog. */
export function Popover({ trigger, children, align = "center", label }: {
  trigger: ReactNode;
  children: ReactNode;
  align?: "start" | "center" | "end";
  label: string;
}) {
  const container = useOverlayContainer();
  const [open, setOpen] = useOverlayOpen();
  return (
    <PopoverPrimitive.Root open={open} onOpenChange={setOpen}>
      <PopoverPrimitive.Trigger asChild>{trigger}</PopoverPrimitive.Trigger>
      <PopoverPrimitive.Portal container={container}>
        <PopoverPrimitive.Content
          className="ui-overlay ui-popover"
          data-surface="well"
          align={align}
          sideOffset={6}
          aria-label={label}
        >
          {children}
        </PopoverPrimitive.Content>
      </PopoverPrimitive.Portal>
    </PopoverPrimitive.Root>
  );
}
