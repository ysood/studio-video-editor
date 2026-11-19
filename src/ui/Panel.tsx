import { cva } from "class-variance-authority";
import type { ReactNode } from "react";
import { cn } from "../lib/cn";
import type { SurfaceName } from "../theme";

/**
 * A dock panel, and the one anatomy every panel has:
 *
 *   header   `PanelToolbar`, or `Tabs fill`, or the dock's own title strip
 *   body     `PanelBody`, or the panel's own scrolling content
 *   footer   `PanelToolbar placement="bottom"`, when there is one
 *
 * The header and footer are the same bar everywhere: `size.toolbar` tall, the panel
 * inset (`space[6]`) at the sides, and a hairline against the body. Whatever a panel puts
 * in its body starts at that same inset, so the first thing in a toolbar and the first
 * thing under it share a left edge in every panel. That shared edge is most of what
 * makes four panels read as one product; four panels each with their own bar height
 * and inset is what makes them read as four.
 *
 * `text-label` here is the type scale's rule that the interface is set at one size.
 */
export function Panel({ children, className, surface = "panel" }: {
  children?: ReactNode;
  className?: string;
  surface?: SurfaceName;
}) {
  return (
    <div className={cn("flex h-full flex-col overflow-hidden bg-panel text-label", className)} data-surface={surface}>
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
 * Every panel bar: one height, the panel inset at the sides, and a hairline on the edge
 * that faces the body. Use Gap between groups. Positioned, so a bar can park something
 * at its edge.
 *
 * No margin around it. The bar's height already leaves its controls clear of both of
 * its edges, and a bar floated off the top of its panel by a margin is a band of empty
 * panel that every panel then has to agree on.
 */
const toolbarVariants = cva("relative flex flex-none items-center gap-2 px-6", {
  variants: {
    placement: {
      top: "h-toolbar border-b border-edge",
      bottom: "h-panel-footer border-t border-edge",
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
      // Separate wells, at the panel inset and one gap apart.
      wells: "flex flex-col gap-6 p-6 [&>[data-note]]:mx-0 [&>[data-note]]:my-2",
      // One surface: sections run together and are told apart by a divider. A section
      // brings the panel inset itself, so its fields line up under the header's
      // contents. A divider earns its place only between two sections, so the last one
      // keeps no line.
      sheet: "[&>section:not(:has(~section))]:border-b-0",
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
    <div className={cn(bodyVariants({ layout }), className)}>
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
    <div className="grid h-full place-content-center justify-items-center gap-5 p-6 text-center text-fg-muted">
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
        "border-b border-edge p-6 [&_[data-note]]:mx-0 [&_[data-note]]:my-2",
        // An inset section is a well; the panel body spaces wells, so no margins here.
        "data-[surface]:rounded-panel data-[surface]:border-b-0 data-[surface]:bg-panel",
      )}
      data-surface={surface}
    >
      <h2 className="mb-5 text-heading text-fg-heading">{title}</h2>
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
