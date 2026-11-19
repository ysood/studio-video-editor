import * as PopoverPrimitive from "@radix-ui/react-popover";
import type { ReactNode } from "react";
import { useOverlayContainer, useOverlayOpen } from "./overlay";
import { cn } from "../lib/cn";
import { overlayMenuPanel } from "./overlayStyles";

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
          // Content, not rows, so it takes the roomier inset a menu does not.
          className={cn(overlayMenuPanel, "p-6 text-label")}
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
