import { useEffect, useState } from "react";
import type { Snapshot } from "./bindings/Snapshot";
import type { ToolsStatus } from "./bindings/ToolsStatus";
import { errorMessage, ipc } from "./ipc";

// Placeholder that proves the IPC round trip; replaced by the workspace in the next layer.
export function App() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [tools, setTools] = useState<ToolsStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([ipc.snapshot(), ipc.toolsStatus()])
      .then(([s, t]) => {
        setSnapshot(s);
        setTools(t);
      })
      .catch((e: unknown) => setError(errorMessage(e)));
  }, []);

  return (
    <main style={{ fontFamily: "system-ui", padding: 24 }}>
      <h1>Studio</h1>
      {error && <p role="alert">{error}</p>}
      {snapshot && <p>Project: {snapshot.project.name}</p>}
      {tools && (
        <p>
          ffmpeg: {tools.ffmpeg ?? "not found"} · ffprobe: {tools.ffprobe ?? "not found"}
        </p>
      )}
    </main>
  );
}
