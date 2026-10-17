import {
  DockviewReact,
  themeDark,
  type DockviewApi,
  type DockviewReadyEvent,
  type IDockviewPanelProps,
} from "dockview-react";
import "dockview-react/dist/styles/dockview.css";
import type { FunctionComponent } from "react";
import { InspectorPanel } from "../panels/InspectorPanel";
import { MediaBinPanel } from "../panels/MediaBinPanel";
import { TimelinePanel } from "../panels/TimelinePanel";
import { ViewerPanel } from "../panels/ViewerPanel";

// Bump when panels are added or renamed so stale saved layouts are discarded.
const LAYOUT_KEY = "studio.layout.v1";

const components: Record<string, FunctionComponent<IDockviewPanelProps>> = {
  bin: MediaBinPanel,
  viewer: ViewerPanel,
  inspector: InspectorPanel,
  timeline: TimelinePanel,
};

function defaultLayout(api: DockviewApi) {
  api.addPanel({ id: "viewer", component: "viewer", title: "Viewer" });
  api.addPanel({
    id: "timeline",
    component: "timeline",
    title: "Timeline",
    position: { direction: "below" },
    initialHeight: 360,
  });
  api.addPanel({
    id: "bin",
    component: "bin",
    title: "Media",
    position: { referencePanel: "viewer", direction: "left" },
    initialWidth: 320,
  });
  api.addPanel({
    id: "inspector",
    component: "inspector",
    title: "Inspector",
    position: { referencePanel: "viewer", direction: "right" },
    initialWidth: 300,
  });
}

function onReady(event: DockviewReadyEvent) {
  const { api } = event;
  let restored = false;
  try {
    const saved = localStorage.getItem(LAYOUT_KEY);
    if (saved) {
      api.fromJSON(JSON.parse(saved));
      restored = Object.keys(components).every((id) => api.getPanel(id));
    }
  } catch {
    restored = false;
  }
  if (!restored) {
    api.clear();
    defaultLayout(api);
  }
  api.onDidLayoutChange(() => {
    try {
      localStorage.setItem(LAYOUT_KEY, JSON.stringify(api.toJSON()));
    } catch {
      // Storage can be unavailable; the default layout is fine.
    }
  });
}

export function resetLayout() {
  try {
    localStorage.removeItem(LAYOUT_KEY);
  } catch {
    // ignore
  }
  window.location.reload();
}

export function Workspace() {
  return (
    <div className="dock" data-surface="chrome">
      <DockviewReact components={components} onReady={onReady} theme={themeDark} />
    </div>
  );
}
