import { useEffect } from "react";
import { runAction } from "./actions";
import { events } from "./ipc";
import { useGlobalShortcuts } from "./keyboard";
import { useEditor } from "./store";
import { ExportDialog } from "./workspace/ExportDialog";
import { MatchSequenceDialog } from "./workspace/MatchSequenceDialog";
import { StatusBar } from "./workspace/StatusBar";
import { TopBar } from "./workspace/TopBar";
import { Workspace } from "./workspace/Workspace";

export function App() {
  const init = useEditor((s) => s.init);
  const exportOpen = useEditor((s) => s.exportOpen);
  const setExportOpen = useEditor((s) => s.setExportOpen);

  useEffect(() => {
    void init();
  }, [init]);

  // Events from Rust: background jobs and native menu choices.
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
      events.onMenuAction((action) => runAction(action, "menu")),
    ];
    return () => {
      for (const s of subscriptions) void s.then((unlisten) => unlisten());
    };
  }, []);

  useGlobalShortcuts();

  return (
    <div className="app">
      <TopBar />
      <Workspace />
      <StatusBar />
      <ExportDialog open={exportOpen} onClose={() => setExportOpen(false)} />
      <MatchSequenceDialog />
    </div>
  );
}
