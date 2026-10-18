import { useEffect, useState } from "react";
import { runAction } from "./actions";
import { events } from "./ipc";
import { useGlobalShortcuts } from "./keyboard";
import { useEditor } from "./store";
import { ExportDialog } from "./workspace/ExportDialog";
import { HomeScreen } from "./workspace/HomeScreen";
import { MatchSequenceDialog } from "./workspace/MatchSequenceDialog";
import { NewProjectDialog } from "./workspace/NewProjectDialog";
import { StatusBar } from "./workspace/StatusBar";
import { TopBar } from "./workspace/TopBar";
import { Workspace } from "./workspace/Workspace";

export function App() {
  const init = useEditor((s) => s.init);
  const exportOpen = useEditor((s) => s.exportOpen);
  const setExportOpen = useEditor((s) => s.setExportOpen);
  const hasProject = useEditor((s) => s.snapshot !== null);
  const ready = useEditor((s) => s.ready);
  const [newProjectOpen, setNewProjectOpen] = useState(false);

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

  // Nothing until the first snapshot has been asked for, so the home screen does not
  // appear for a frame on the way into a project.
  if (!ready) return <div className="app" />;

  if (!hasProject) {
    return (
      <>
        <HomeScreen onNew={() => setNewProjectOpen(true)} />
        <NewProjectDialog open={newProjectOpen} onClose={() => setNewProjectOpen(false)} />
      </>
    );
  }

  return (
    <div className="app">
      <TopBar />
      <Workspace />
      <StatusBar />
      <ExportDialog open={exportOpen} onClose={() => setExportOpen(false)} />
      <MatchSequenceDialog />
      <NewProjectDialog open={newProjectOpen} onClose={() => setNewProjectOpen(false)} />
    </div>
  );
}
