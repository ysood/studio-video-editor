import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import type { ReactElement, ReactNode } from "react";
import { useTheme } from "../theme";
import { useOverlayContainer } from "./overlay";
import { cn } from "../lib/cn";
import { overlayPanel, overlayShortcut } from "./overlayStyles";

/** Mount once near the root; sets the shared hover delay from the theme. */
export function TooltipProvider({ children }: { children: ReactNode }) {
  const theme = useTheme();
  return (
    <TooltipPrimitive.Provider delayDuration={theme.motion.tooltipDelay} skipDelayDuration={theme.motion.base}>
      {children}
    </TooltipPrimitive.Provider>
  );
}

/** A label shown on hover or keyboard focus. Use instead of `title` attributes. */
export function Tooltip({ label, shortcut, children }: { label: string; shortcut?: string; children: ReactElement }) {
  const container = useOverlayContainer();
  return (
    <TooltipPrimitive.Root>
      <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
      <TooltipPrimitive.Portal container={container}>
        <TooltipPrimitive.Content className={cn(overlayPanel, "flex items-center px-4 py-2 text-caption")} data-surface="well" sideOffset={6}>
          {label}
          {shortcut && <span className={cn(overlayShortcut, "ps-4")}>{shortcut}</span>}
        </TooltipPrimitive.Content>
      </TooltipPrimitive.Portal>
    </TooltipPrimitive.Root>
  );
}
