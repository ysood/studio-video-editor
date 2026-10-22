import { useState } from "react";
import { runAction } from "../actions";
import { formatRate } from "../format";
import { chord } from "../keyboard";
import { useActiveSequence, useEditor } from "../store";
import { toggleChat } from "./layout";
import { Button, IconButton, Menu, type MenuEntry, TextInput } from "../ui";

export function TopBar() {
  const snapshot = useEditor((s) => s.snapshot);
  const nativeMenu = useEditor((s) => s.nativeMenu);
  const edit = useEditor((s) => s.edit);
  const sequence = useActiveSequence();
  // Renamed in place, the way the timelines in the bin are: the title itself becomes
  // the field, so there is no dialog to say what is being renamed.
  const [renaming, setRenaming] = useState(false);
  const history = snapshot?.history;
  const format = sequence?.format;
  const hasClips =
    sequence !== null && sequence.tracks.some((t) => t.clips.length > 0);

  const fileMenu: MenuEntry[] = [
    { label: "New project", icon: "file", shortcut: chord("N"), onSelect: () => runAction("newProject") },
    { label: "Open…", icon: "folder", shortcut: chord("O"), onSelect: () => runAction("openProject") },
    { label: "Close project", icon: "close", shortcut: chord("W"), onSelect: () => runAction("closeProject") },
    { type: "separator" },
    { label: "Save", shortcut: chord("S"), disabled: !snapshot, onSelect: () => runAction("save") },
    {
      label: "Save as…",
      shortcut: chord("S", { shift: true }),
      disabled: !snapshot,
      onSelect: () => runAction("saveAs"),
    },
    { type: "separator" },
    { label: "Import media…", icon: "plus", shortcut: chord("I"), onSelect: () => runAction("importMedia") },
    { label: "Export…", icon: "export", shortcut: chord("E"), disabled: !hasClips, onSelect: () => runAction("export") },
  ];

  return (
    <header className="topbar" data-surface="chrome">
      {/* On macOS these live in the system menu bar instead. */}
      {!nativeMenu && <Menu trigger={<Button variant="ghost">File</Button>} entries={fileMenu} />}
      <div className="topbar-title">
        {renaming && snapshot ? (
          <TextInput
            focused
            value={snapshot.project.name}
            ariaLabel="Project name"
            onCommit={(name) => void edit({ type: "RenameProject", name })}
            onDone={() => setRenaming(false)}
          />
        ) : (
          <strong
            className="topbar-name"
            title={snapshot ? "Click to rename" : undefined}
            onClick={() => snapshot && setRenaming(true)}
          >
            {snapshot?.project.name ?? "Studio"}
          </strong>
        )}
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
          onClick={() => runAction("undo")}
        />
        <IconButton
          icon="redo"
          label={history?.redoLabel ? `Redo ${history.redoLabel}` : "Redo"}
          shortcut={chord("Z", { shift: true })}
          disabled={!history?.canRedo}
          onClick={() => runAction("redo")}
        />
      </div>
      <IconButton icon="chat" label="Assistant" onClick={toggleChat} />
      <Button variant="solid" icon="export" onClick={() => runAction("export")} disabled={!hasClips}>
        Export
      </Button>
    </header>
  );
}
