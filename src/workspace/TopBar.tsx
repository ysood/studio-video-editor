import { importMedia, newProject, openProject, saveProject } from "../actions";
import { formatRate } from "../format";
import { chord } from "../keyboard";
import { useEditor } from "../store";
import { Button, IconButton, Menu, type MenuEntry } from "../ui";

export function TopBar({ onExport }: { onExport: () => void }) {
  const snapshot = useEditor((s) => s.snapshot);
  const undo = useEditor((s) => s.undo);
  const redo = useEditor((s) => s.redo);
  const history = snapshot?.history;
  const format = snapshot?.project.sequence.format;
  const hasClips = snapshot?.project.sequence.tracks.some((t) => t.clips.length > 0) ?? false;

  const fileMenu: MenuEntry[] = [
    { label: "New project", icon: "file", shortcut: chord("N"), onSelect: () => void newProject() },
    { label: "Open…", icon: "folder", shortcut: chord("O"), onSelect: () => void openProject() },
    { type: "separator" },
    { label: "Save", shortcut: chord("S"), disabled: !snapshot, onSelect: () => void saveProject() },
    {
      label: "Save as…",
      shortcut: chord("S", { shift: true }),
      disabled: !snapshot,
      onSelect: () => void saveProject(true),
    },
    { type: "separator" },
    { label: "Import media…", icon: "plus", shortcut: chord("I"), onSelect: () => void importMedia() },
    { label: "Export…", icon: "export", shortcut: chord("E"), disabled: !hasClips, onSelect: onExport },
  ];

  return (
    <header className="topbar" data-surface="chrome">
      <Menu trigger={<Button variant="ghost">File</Button>} entries={fileMenu} />
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
        <IconButton
          icon="undo"
          label={history?.undoLabel ? `Undo ${history.undoLabel}` : "Undo"}
          shortcut={chord("Z")}
          disabled={!history?.canUndo}
          onClick={() => void undo()}
        />
        <IconButton
          icon="redo"
          label={history?.redoLabel ? `Redo ${history.redoLabel}` : "Redo"}
          shortcut={chord("Z", { shift: true })}
          disabled={!history?.canRedo}
          onClick={() => void redo()}
        />
      </div>
      <Button variant="solid" icon="export" onClick={onExport} disabled={!hasClips}>
        Export
      </Button>
    </header>
  );
}
