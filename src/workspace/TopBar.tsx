import { importMedia, newProject, openProject, saveProject } from "../actions";
import { useEditor } from "../store";

export function TopBar({ onExport }: { onExport: () => void }) {
  const snapshot = useEditor((s) => s.snapshot);
  const undo = useEditor((s) => s.undo);
  const redo = useEditor((s) => s.redo);
  const history = snapshot?.history;
  const format = snapshot?.project.sequence.format;
  const hasClips = snapshot?.project.sequence.tracks.some((t) => t.clips.length > 0) ?? false;

  return (
    <header className="topbar">
      <div className="topbar-title">
        <strong>{snapshot?.project.name ?? "Studio"}</strong>
        {history?.dirty && <span className="dirty-dot" title="Unsaved changes" />}
        {format && (
          <span className="tabular">
            {format.width}×{format.height}, {(format.frameRate.num / format.frameRate.den).toFixed(3).replace(/\.?0+$/, "")} fps
          </span>
        )}
      </div>
      <div className="topbar-group">
        <button className="btn" onClick={newProject}>New</button>
        <button className="btn" onClick={openProject}>Open</button>
        <button className="btn" onClick={() => saveProject()}>Save</button>
      </div>
      <div className="topbar-group">
        <button
          className="btn"
          onClick={undo}
          disabled={!history?.canUndo}
          title={history?.undoLabel ? `Undo ${history.undoLabel}` : "Nothing to undo"}
        >
          Undo
        </button>
        <button
          className="btn"
          onClick={redo}
          disabled={!history?.canRedo}
          title={history?.redoLabel ? `Redo ${history.redoLabel}` : "Nothing to redo"}
        >
          Redo
        </button>
      </div>
      <div className="topbar-group">
        <button className="btn" onClick={importMedia}>Import</button>
        <button className="btn btn-primary" onClick={onExport} disabled={!hasClips}>
          Export
        </button>
      </div>
    </header>
  );
}
