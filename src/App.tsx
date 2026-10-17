import { useCallback, useEffect, useMemo, useState } from "react";
import { events } from "./ipc";
import { useGlobalShortcuts, useShortcut } from "./keyboard";
import { useEditor } from "./store";
import { ExportDialog } from "./workspace/ExportDialog";
import { StatusBar } from "./workspace/StatusBar";
import { TopBar } from "./workspace/TopBar";
import { Workspace } from "./workspace/Workspace";

export function App() {
  const init = useEditor((s) => s.init);
  const undo = useEditor((s) => s.undo);
  const redo = useEditor((s) => s.redo);
  const [exportOpen, setExportOpen] = useState(false);

  useEffect(() => {
    void init();
  }, [init]);

  // Background job events from Rust.
  useEffect(() => {
    const { setJob, notify } = useEditor.getState();
    const subscriptions = [
      events.onJobProgress((p) => setJob({ kind: p.kind, fraction: p.fraction })),
      events.onJobFinished((f) => {
        setJob(null);
        const what = f.kind === "render" ? "Render" : "Package";
        if (f.ok) notify("info", `${what} finished: ${f.output}`);
        else if (f.cancelled) notify("info", `${what} cancelled`);
        else notify("error", `${what} failed: ${f.message ?? "unknown error"}`);
      }),
    ];
    return () => {
      for (const s of subscriptions) void s.then((unlisten) => unlisten());
    };
  }, []);

  const actions = useMemo(() => ({ undo: () => void undo(), redo: () => void redo() }), [undo, redo]);
  useGlobalShortcuts(actions);
  useShortcut(
    useCallback((e: KeyboardEvent) => {
      if (!((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "e")) return false;
      setExportOpen(true);
      return true;
    }, []),
  );

  return (
    <div className="app">
      <TopBar onExport={() => setExportOpen(true)} />
      <Workspace />
      <StatusBar />
      <ExportDialog open={exportOpen} onClose={() => setExportOpen(false)} />
    </div>
  );
}
