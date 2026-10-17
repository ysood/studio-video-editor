import type { ReactNode } from "react";
import "./panel.css";

export function Panel({ children, className }: { children?: ReactNode; className?: string }) {
  return <div className={className ? `ui-panel ${className}` : "ui-panel"}>{children}</div>;
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
export function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="ui-section">
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
