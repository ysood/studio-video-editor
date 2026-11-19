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

/**
 * Text tabs with arrow-key navigation; the selected tab's content renders below.
 *
 * `fill` makes the strip the panel's own chrome: it sits flush at the top edge taking
 * the card inset at the sides, and the chosen tab's content takes the rest of the
 * height and scrolls. Use it for a panel that is two views of one thing, rather than a
 * toolbar holding a control that switches them. A pill floating in the middle of an
 * otherwise empty band does not read as "these are the two views of this panel"; it
 * reads as a cutout nobody can explain.
 */
export function Tabs({ value, onChange, tabs, label, fill }: {
  value: string;
  onChange: (value: string) => void;
  tabs: TabSpec[];
  label: string;
  fill?: boolean;
}) {
  return (
    <TabsPrimitive.Root
      className={cn(fill && "flex min-h-0 flex-1 flex-col")}
      value={value}
      onValueChange={onChange}
    >
      <TabsPrimitive.List
        // Filled, the strip is the panel's header bar: the same height and inset as a
        // toolbar, so a panel with tabs and a panel with a toolbar line up.
        className={cn("flex gap-5 border-b border-edge", fill && "h-toolbar flex-none gap-7 px-6")}
        aria-label={label}
      >
        {tabs.map((t) => (
          <TabsPrimitive.Trigger
            key={t.value}
            value={t.value}
            className={cn(
              "cursor-default border-0 bg-transparent p-0 text-label text-fg-muted outline-none",
              // In a header bar the tab is as tall as the bar and overlaps its rule by a
              // hairline, so the underline of the chosen tab replaces the rule under it
              // rather than floating above it.
              fill ? "-mb-hairline h-toolbar" : "h-control",
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
        <TabsPrimitive.Content
          key={t.value}
          value={t.value}
          // A filled tab owns the rest of the panel, and its content brings its own
          // inset, so there is no padding to add here.
          className={cn(fill ? "min-h-0 flex-1 overflow-hidden" : "pt-4")}
        >
          {t.content}
        </TabsPrimitive.Content>
      ))}
    </TabsPrimitive.Root>
  );
}

const segmentedVariants = cva("inline-flex h-control rounded-button bg-raised p-hairline", {
  variants: {
    emphasis: {
      quiet: "",
      // Ranged left with everything else in the toolbar. It used to be centred, on the
      // grounds that it is the thing the panel is about, and that was a mistake: a
      // control alone in the middle of a wide empty bar reads as decoration rather than
      // as the panel's main choice. It may still shrink and scroll, because in a narrow
      // panel sliding the choices beats crushing whatever shares the toolbar with them.
      accent: "min-w-0 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
    },
  },
  defaultVariants: { emphasis: "quiet" },
});

const segmentVariants = cva(
  cn(
    controlBase,
    // The height is the container's, less its inset, so a segment stretches to it rather
    // than restating a calc; an icon-only one is then square by aspect ratio.
    "rounded-button bg-transparent px-5 text-label text-fg-muted hover:text-fg",
    "data-[icon-only]:aspect-square data-[icon-only]:px-0",
  ),
  {
    variants: {
      emphasis: {
        // Lifts: the selected fill layers over the raised track.
        quiet: "data-[state=on]:state-selected data-[state=on]:text-fg",
        // Filled the way a primary button is.
        accent: "data-[state=on]:bg-accent data-[state=on]:text-fg-on-accent",
      },
    },
    defaultVariants: { emphasis: "quiet" },
  },
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
 */
export function Segmented({ value, options, onChange, label, emphasis = "quiet", className }: {
  value: string;
  options: SegmentOption[];
  onChange: (value: string) => void;
  label: string;
  className?: string;
  /**
   * "accent" fills the chosen segment, for the control that is a panel's main choice.
   * "quiet" lifts it instead, for a secondary one such as a view toggle. Two accented
   * groups in one toolbar would both claim to be the main thing.
   */
  emphasis?: "quiet" | "accent";
}) {
  return (
    <ToggleGroupPrimitive.Root
      type="single"
      className={cn(segmentedVariants({ emphasis }), className)}
      value={value}
      aria-label={label}
      onValueChange={(next) => next && onChange(next)}
    >
      {options.map((o) => {
        const item = (
          <ToggleGroupPrimitive.Item
            key={o.value}
            value={o.value}
            className={segmentVariants({ emphasis })}
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
