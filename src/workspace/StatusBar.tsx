import { useEditor } from "../store";

export function StatusBar() {
  const status = useEditor((s) => s.status);
  const job = useEditor((s) => s.job);
  const snapshot = useEditor((s) => s.snapshot);

  return (
    <footer className="statusbar" role="status">
      {status && <span className={status.kind === "error" ? "error" : undefined}>{status.text}</span>}
      <span className="push" />
      {job && (
        <>
          <span>{job.kind === "render" ? "Rendering" : "Packaging"}</span>
          <div className="progress" aria-hidden>
            <div style={{ width: `${Math.round(job.fraction * 100)}%` }} />
          </div>
          <span className="tabular">{Math.round(job.fraction * 100)}%</span>
        </>
      )}
      {snapshot && <span>{snapshot.path ?? "Not saved yet"}</span>}
    </footer>
  );
}
