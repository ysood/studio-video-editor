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

export function PanelToolbar({ children }: { children: ReactNode }) {
  return <div className="ui-panel-toolbar">{children}</div>;
}

export function PanelBody({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={className ? `ui-panel-body ${className}` : "ui-panel-body"}>{children}</div>;
}

/** Pushes following toolbar items to the far end. */
export function Spacer() {
  return <span className="ui-spacer" />;
}

/** A small fixed gap between toolbar groups. */
export function Gap() {
  return <span className="ui-gap" />;
}

/** Guidance shown where content will appear, with an optional action. */
export function EmptyState({ children, action }: { children: ReactNode; action?: ReactNode }) {
  return (
    <div className="ui-empty">
      <p>{children}</p>
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
