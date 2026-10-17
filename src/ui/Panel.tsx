import type { ReactNode } from "react";
import type { SurfaceName } from "../theme";
import "./panel.css";

/** A dock panel. `surface` picks its color context (see SurfaceName). */
export function Panel({ children, className, surface = "panel" }: {
  children?: ReactNode;
  className?: string;
  surface?: SurfaceName;
}) {
  return (
    <div className={className ? `ui-panel ${className}` : "ui-panel"} data-surface={surface}>
      {children}
    </div>
  );
}

/** An inset content region inside a panel, on the "well" surface. */
export function Well({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div className={className ? `ui-well ${className}` : "ui-well"} data-surface="well">
      {children}
    </div>
  );
}

/** A row of controls at the top or bottom edge of a panel. */
export function PanelToolbar({ children, placement = "top" }: { children: ReactNode; placement?: "top" | "bottom" }) {
  return (
    <div className="ui-panel-toolbar" data-placement={placement}>
      {children}
    </div>
  );
}

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
      className={className ? `ui-panel-body ${className}` : "ui-panel-body"}
      data-layout={layout}
      data-surface={layout === "sheet" ? "well" : undefined}
    >
      {children}
    </div>
  );
}

/** Pushes following toolbar items to the far end. */
export function Spacer() {
  return <span className="ui-spacer" />;
}

/** A small fixed gap between toolbar groups. */
export function Gap() {
  return <span className="ui-gap" />;
}

/**
 * Guidance shown where content will appear, with an optional action. Either half may
 * stand alone: a space whose emptiness speaks for itself needs the action and no words.
 */
export function EmptyState({ children, action }: { children?: ReactNode; action?: ReactNode }) {
  return (
    <div className="ui-empty">
      {children && <p>{children}</p>}
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
    <section className="ui-section" data-surface={surface}>
      <h2>{title}</h2>
      {children}
    </section>
  );
}

export function Note({ children }: { children: ReactNode }) {
  return <p className="ui-note">{children}</p>;
}

export function Actions({ children, align = "start" }: { children: ReactNode; align?: "start" | "end" }) {
  return (
    <div className="ui-actions" data-align={align}>
      {children}
    </div>
  );
}

export function ProgressBar({ fraction }: { fraction: number }) {
  const percent = Math.round(Math.min(1, Math.max(0, fraction)) * 100);
  return (
    <div className="ui-progress" role="progressbar" aria-valuenow={percent} aria-valuemin={0} aria-valuemax={100}>
      <div style={{ width: `${percent}%` }} />
    </div>
  );
}
