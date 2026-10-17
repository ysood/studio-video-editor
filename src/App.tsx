import { useEffect, useMemo, useState } from "react";
import { useGlobalShortcuts } from "./keyboard";
import { useEditor } from "./store";
import { StatusBar } from "./workspace/StatusBar";
import { TopBar } from "./workspace/TopBar";
import { Workspace } from "./workspace/Workspace";

export function App() {
  const init = useEditor((s) => s.init);
  const undo = useEditor((s) => s.undo);
  const redo = useEditor((s) => s.redo);
  const [, setExportOpen] = useState(false);

  useEffect(() => {
    void init();
  }, [init]);

  const actions = useMemo(() => ({ undo: () => void undo(), redo: () => void redo() }), [undo, redo]);
  useGlobalShortcuts(actions);

  return (
    <div className="app">
      <TopBar onExport={() => setExportOpen(true)} />
      <Workspace />
      <StatusBar />
    </div>
  );
}
