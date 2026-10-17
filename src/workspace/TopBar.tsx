import { importMedia, newProject, openProject, saveProject } from "../actions";
import { formatRate } from "../format";
import { useEditor } from "../store";
import { Button } from "../ui";

export function TopBar({ onExport }: { onExport: () => void }) {
  const snapshot = useEditor((s) => s.snapshot);
  const undo = useEditor((s) => s.undo);
  const redo = useEditor((s) => s.redo);
  const history = snapshot?.history;
  const format = snapshot?.project.sequence.format;
  const hasClips = snapshot?.project.sequence.tracks.some((t) => t.clips.length > 0) ?? false;

  return (
    <header className="topbar" data-surface="chrome">
      <div className="topbar-title">
        <strong>{snapshot?.project.name ?? "Studio"}</strong>
        {history?.dirty && <span className="dirty-dot" title="Unsaved changes" />}
        {format && (
          <span className="numeric">
            {format.width}×{format.height}, {formatRate(format.frameRate)} fps
          </span>
        )}
      </div>
      <div className="topbar-group">
        <Button onClick={newProject}>New</Button>
        <Button onClick={openProject}>Open</Button>
        <Button onClick={() => saveProject()}>Save</Button>
      </div>
      <div className="topbar-group">
        <Button
          onClick={undo}
          disabled={!history?.canUndo}
          title={history?.undoLabel ? `Undo ${history.undoLabel}` : "Nothing to undo"}
        >
          Undo
        </Button>
        <Button
          onClick={redo}
          disabled={!history?.canRedo}
          title={history?.redoLabel ? `Redo ${history.redoLabel}` : "Nothing to redo"}
        >
          Redo
        </Button>
      </div>
      <div className="topbar-group">
        <Button onClick={importMedia}>Import</Button>
        <Button variant="primary" onClick={onExport} disabled={!hasClips}>
          Export
        </Button>
      </div>
    </header>
  );
}
