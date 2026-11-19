import { cva } from "class-variance-authority";
import type { ReactNode } from "react";
import { cn } from "../lib/cn";
import type { SurfaceName } from "../theme";

/** A dock panel. `surface` picks its color context (see SurfaceName). */
export function Panel({ children, className, surface = "panel" }: {
  children?: ReactNode;
  className?: string;
  surface?: SurfaceName;
}) {
  return (
    <div className={cn("flex h-full flex-col overflow-hidden bg-panel", className)} data-surface={surface}>
      {children}
    </div>
  );
}

/** An inset content region inside a panel, on the "well" surface. */
export function Well({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div className={cn("rounded-panel bg-panel", className)} data-surface="well">
      {children}
    </div>
  );
}

/**
 * Every toolbar: same height, padding and spacing. Use Gap between groups. Positioned,
 * so a toolbar can park something at its edge without that thing taking part in the
 * centring of what sits in the middle.
 *
 * The card inset applies on every side, so a toolbar stands off the edge it faces by
 * the same amount it stands off the sides. At the edge it is a margin rather than
 * padding, so `size.toolbar` keeps meaning the height of the row of controls and not
 * the height of the row plus the air around it.
 */
const toolbarVariants = cva("relative flex flex-none items-center gap-2 px-8", {
  variants: {
    placement: {
      top: "mt-8 h-toolbar",
      // Bottom strips line up across panels, whatever they contain.
      bottom: "mb-8 h-panel-footer",
    },
  },
  defaultVariants: { placement: "top" },
});

/** A row of controls at the top or bottom edge of a panel. */
export function PanelToolbar({ children, placement = "top" }: { children: ReactNode; placement?: "top" | "bottom" }) {
  return (
    <div className={toolbarVariants({ placement })}>
      {children}
    </div>
  );
}

const bodyVariants = cva("min-h-0 flex-1 overflow-auto", {
  variants: {
    layout: {
      // One inset and one gap between wells, everywhere. The inset is the card inset,
      // and every other edge inside a card lines up with it.
      wells: "flex flex-col gap-7 p-8 [&>[data-note]]:mx-0 [&>[data-note]]:my-2",
      // One surface instead of a stack of cards: sections are told apart by a divider,
      // which is quieter than a gap that lets the panel colour through between them.
      // The inset is padding rather than a margin so the scrollbar stays at the panel's
      // edge where the media's does. Sections therefore add only vertical rhythm: side
      // padding on them would inset the inset, and the fields would no longer line up
      // with the toolbar above. A divider earns its place only between two sections, so
      // the last one keeps no line.
      sheet: cn(
        "rounded-panel bg-panel p-8",
        "[&>section]:px-0 [&_[data-note]]:mx-0",
        "[&>section:not(:has(~section))]:border-b-0",
      ),
    },
  },
});

/**
 * Scrolling panel content. `layout="wells"` stacks separate wells with the standard
 * inset and gap. `layout="sheet"` makes the body itself one well, so the sections
 * inside run together on a single surface and are told apart by a divider.
 */
export function PanelBody({ children, className, layout }: {
  children: ReactNode;
  className?: string;
  layout?: "wells" | "sheet";
}) {
  return (
    <div
      className={cn(bodyVariants({ layout }), className)}
      data-surface={layout === "sheet" ? "well" : undefined}
    >
      {children}
    </div>
  );
}

/** Pushes following toolbar items to the far end. */
export function Spacer() {
  return <span className="flex-1" />;
}

/** A small fixed gap between toolbar groups. */
export function Gap() {
  return <span className="w-4 flex-none" />;
}


/**
 * Guidance shown where content will appear, with an optional action. Either half may
 * stand alone: a space whose emptiness speaks for itself needs the action and no words.
 */
export function EmptyState({ children, action }: { children?: ReactNode; action?: ReactNode }) {
  return (
    <div className="grid h-full place-content-center justify-items-center gap-5 p-7 text-center text-fg-muted">
      {children && <p className="measure">{children}</p>}
      {action && <div>{action}</div>}
    </div>
  );
}

/** A titled group inside a panel, such as an inspector section. */
export function Section({ title, children, surface }: {
  title: string;
  children: ReactNode;
  /** Set to "well" to inset the section as its own block. */
  surface?: SurfaceName;
}) {
  return (
    <section
      className={cn(
        "border-b border-edge px-6 pb-6 pt-5 [&_[data-note]]:mx-0 [&_[data-note]]:my-2",
        // An inset section is a well; the panel body spaces wells, so no margins here.
        "data-[surface]:rounded-panel data-[surface]:border-b-0 data-[surface]:bg-panel",
      )}
      data-surface={surface}
    >
      <h2 className="mb-4 text-heading text-fg-heading">{title}</h2>
      {children}
    </section>
  );
}

export function Note({ children }: { children: ReactNode }) {
  return <p className="mx-6 mt-4 text-caption text-fg-faint" data-note>{children}</p>;
}

export function Actions({ children, align = "start" }: { children: ReactNode; align?: "start" | "end" }) {
  return (
    <div className={cn("flex flex-wrap gap-3", align === "end" && "justify-end pt-7")}>
      {children}
    </div>
  );
}

export function ProgressBar({ fraction }: { fraction: number }) {
  const percent = Math.round(Math.min(1, Math.max(0, fraction)) * 100);
  return (
    <div
      className="h-progress-height w-progress-width overflow-hidden rounded-round bg-raised"
      role="progressbar"
      aria-valuenow={percent}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <div className="h-full bg-accent transition-all duration-base ease-standard" style={{ width: `${percent}%` }} />
    </div>
  );
}
