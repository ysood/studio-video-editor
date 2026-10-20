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
import { TransformPanel } from "../panels/TransformPanel";
import { ViewerPanel } from "../panels/ViewerPanel";
import { importMedia } from "../actions";
import { chord } from "../keyboard";
import { useEditor } from "../store";
import { LAYOUT_KEY, resetLayout } from "./layout";
import { makeHeaderActions, PanelTab } from "./PanelHeader";
import { activeSequence } from "../sequences";


const components: Record<string, FunctionComponent<IDockviewPanelProps>> = {
  bin: MediaBinPanel,
  viewer: ViewerPanel,
  inspector: InspectorPanel,
  transform: TransformPanel,
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
    initialWidth: 440,
  });
  api.addPanel({
    id: "inspector",
    component: "inspector",
    title: "Inspector",
    position: { referencePanel: "viewer", direction: "right" },
    initialWidth: 300,
  });
  // No direction, so it joins the inspector's group as a second tab rather than taking
  // room of its own. The two answer different questions about the same selection.
  api.addPanel({
    id: "transform",
    component: "transform",
    title: "Transform",
    position: { referencePanel: "inspector" },
    inactive: true,
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

const HeaderActions = makeHeaderActions(
  {
    bin: () => [{ label: "Import media…", icon: "plus", shortcut: chord("I"), onSelect: () => void importMedia() }],
    timeline: () => {
      const { edit, snapshot } = useEditor.getState();
      if (!snapshot) return [];
      const sequenceId = activeSequence(snapshot.project, useEditor.getState().activeSequenceId).id;
      return [
        { label: "Add video track", icon: "video", onSelect: () => void edit({ type: "AddTrack", sequenceId, kind: "video" }) },
        { label: "Add audio track", icon: "audio", onSelect: () => void edit({ type: "AddTrack", sequenceId, kind: "audio" }) },
      ];
    },
  },
  () => [{ label: "Reset layout", icon: "grid", onSelect: resetLayout }],
);

export function Workspace() {
  return (
    <div className="dock" data-surface="chrome">
      <DockviewReact
        components={components}
        defaultTabComponent={PanelTab}
        rightHeaderActionsComponent={HeaderActions}
        onReady={onReady}
        theme={themeDark}
      />
    </div>
  );
}
