import {
  DockviewReact,
  themeDark,
  type DockviewApi,
  type DockviewReadyEvent,
  type IDockviewPanelProps,
} from "dockview-react";
import "dockview-react/dist/styles/dockview.css";
import type { FunctionComponent } from "react";
import { MediaBinPanel } from "../panels/MediaBinPanel";
import { TimelinePanel } from "../panels/TimelinePanel";
import { PropertiesPanel } from "../panels/PropertiesPanel";
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
  inspector: PropertiesPanel,
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
    // Wide enough for the bin pills to sit in one row beside the view buttons; the
    // media wants the room too, now that nothing stands to the left of it.
    initialWidth: 500,
  });
  // The inspector and the transform are one panel with a pill between them, so there
  // is nothing to add for the second: they answer different questions about the same
  // selection and never wanted room of their own.
  api.addPanel({
    id: "inspector",
    component: "inspector",
    title: "Inspector",
    position: { referencePanel: "viewer", direction: "right" },
    initialWidth: 320,
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
