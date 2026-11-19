import * as TabsPrimitive from "@radix-ui/react-tabs";
import * as ToggleGroupPrimitive from "@radix-ui/react-toggle-group";
import { cva } from "class-variance-authority";
import type { ReactNode } from "react";
import { cn } from "../lib/cn";
import { controlBase } from "./Button";
import { Icon, type IconName } from "./Icon";
import { Tooltip } from "./Tooltip";

export interface TabSpec {
  value: string;
  label: string;
  content: ReactNode;
}

/** Text tabs with arrow-key navigation; the selected tab's content renders below. */
export function Tabs({ value, onChange, tabs, label }: {
  value: string;
  onChange: (value: string) => void;
  tabs: TabSpec[];
  label: string;
}) {
  return (
    <TabsPrimitive.Root value={value} onValueChange={onChange}>
      <TabsPrimitive.List className="flex gap-5 border-b border-edge" aria-label={label}>
        {tabs.map((t) => (
          <TabsPrimitive.Trigger
            key={t.value}
            value={t.value}
            className={cn(
              "h-control cursor-default border-0 bg-transparent p-0 text-label text-fg-muted outline-none",
              // The underline is always there, transparent, so choosing a tab never
              // shifts the label by the width of the line.
              "border-b-(length:--border-focus-width) border-b-transparent",
              "transition-colors duration-quick ease-standard hover:text-fg",
              "focus-visible:focus-ring",
              "data-[state=active]:border-b-accent data-[state=active]:text-fg",
            )}
          >
            {t.label}
          </TabsPrimitive.Trigger>
        ))}
      </TabsPrimitive.List>
      {tabs.map((t) => (
        <TabsPrimitive.Content key={t.value} value={t.value} className="pt-4">
          {t.content}
        </TabsPrimitive.Content>
      ))}
    </TabsPrimitive.Root>
  );
}

/**
 * The trough is a field: the panel's black inside a thin outline, like an input, and the
 * chosen segment is the one raised thing in it. That reads as "one of these is in", which a
 * row of buttons on a grey slab does not.
 */
const segmentedVariants = cva("inline-flex h-control rounded-button border border-edge-strong bg-field p-hairline", {
  variants: {
    align: {
      start: "",
      // A panel's view switcher sits in the middle of its header bar. It may shrink and
      // scroll, because in a narrow panel sliding the choices beats crushing whatever
      // shares the bar with them.
      center: "mx-auto min-w-0 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
    },
  },
  defaultVariants: { align: "start" },
});

const segment = cn(
  controlBase,
  // The height is the trough's, less its inset, so a segment stretches to it rather
  // than restating a calc.
  "rounded-control bg-transparent px-5 text-label text-fg-muted hover:text-fg",
  // An explicit width, not `aspect-square`. WebKit resolves an aspect ratio against a
  // stretched flex height as zero width, so in the desktop app every icon-only segment
  // after the first collapsed to nothing and the control showed one icon. Chromium does
  // not, which is why the browser preview never showed it.
  "data-[icon-only]:w-control data-[icon-only]:px-0",
  "data-[state=on]:bg-raised data-[state=on]:text-fg",
);

export interface SegmentOption {
  value: string;
  label: string;
  icon?: IconName;
  /** Show the icon alone, with the label as its tooltip and its accessible name. */
  iconOnly?: boolean;
}

/**
 * A segmented control: pick exactly one of a few options. An option may carry an icon,
 * a label, or both; `iconOnly` drops the text and moves the label into a tooltip, which
 * is what a dense toolbar wants.
 *
 * This is the one way a panel switches between its views. The bin choosing a bin and
 * the properties panel choosing between Inspector and Transform are the same question,
 * so they are the same control in the same place: centred in the header bar. They were
 * a filled pill on one side of the window and underlined text on the other, which is
 * two design languages for one idea.
 */
export function Segmented({ value, options, onChange, label, align, className }: {
  value: string;
  options: SegmentOption[];
  onChange: (value: string) => void;
  label: string;
  className?: string;
  /** "center" for a panel's view switcher; "start" for a choice among other controls. */
  align?: "start" | "center";
}) {
  return (
    <ToggleGroupPrimitive.Root
      type="single"
      className={cn(segmentedVariants({ align }), className)}
      value={value}
      aria-label={label}
      onValueChange={(next) => next && onChange(next)}
    >
      {options.map((o) => {
        const item = (
          <ToggleGroupPrimitive.Item
            key={o.value}
            value={o.value}
            className={segment}
            data-icon-only={o.iconOnly ? "" : undefined}
            aria-label={o.label}
          >
            {o.icon && <Icon name={o.icon} />}
            {!o.iconOnly && o.label}
          </ToggleGroupPrimitive.Item>
        );
        return o.iconOnly ? (
          <Tooltip key={o.value} label={o.label}>
            {item}
          </Tooltip>
        ) : (
          item
        );
      })}
    </ToggleGroupPrimitive.Root>
  );
}
