import {
  DockviewReact,
  themeDark,
  type DockviewApi,
  type DockviewReadyEvent,
  type IDockviewPanelProps,
} from "dockview-react";
import "dockview-react/dist/styles/dockview.css";
import type { FunctionComponent } from "react";
import { ChatPanel } from "../panels/ChatPanel";
import { MediaBinPanel } from "../panels/MediaBinPanel";
import { TimelinePanel } from "../panels/TimelinePanel";
import { PropertiesPanel } from "../panels/PropertiesPanel";
import { ViewerPanel } from "../panels/ViewerPanel";
import { DEFAULT_PANELS, holdDock, LAYOUT_KEY, resetLayout } from "./layout";
import { makeHeaderActions, PanelTab } from "./PanelHeader";


const components: Record<string, FunctionComponent<IDockviewPanelProps>> = {
  bin: MediaBinPanel,
  viewer: ViewerPanel,
  inspector: PropertiesPanel,
  timeline: TimelinePanel,
  chat: ChatPanel,
};

/**
 * Both side panels, so the viewer sits in the middle of the window rather than being
 * pushed off centre by whichever side happens to be wider. It is wide enough for the
 * bin's pills to sit in one row beside its view buttons, and for the properties panel
 * to show a label and its value side by side without the value wrapping.
 */
const SIDE_PANEL = 400;

/**
 * The timeline takes the bottom of the workspace and the three panels above it share
 * the top. A share rather than a pixel count, because what the timeline needs is a
 * share of whatever the window is: 360px was a third of a laptop and a quarter of a
 * desk display, and the tracks are what suffers either way.
 */
const TIMELINE_SHARE = 0.4;

/** Used only if the dock has not been measured yet, which it has by the time this runs. */
const TIMELINE_FALLBACK = 360;

function defaultLayout(api: DockviewApi) {
  api.addPanel({ id: "viewer", component: "viewer", title: "Viewer" });
  api.addPanel({
    id: "timeline",
    component: "timeline",
    title: "Timeline",
    position: { direction: "below" },
    initialHeight: api.height > 0 ? Math.round(api.height * TIMELINE_SHARE) : TIMELINE_FALLBACK,
  });
  api.addPanel({
    id: "bin",
    component: "bin",
    title: "Media",
    position: { referencePanel: "viewer", direction: "left" },
    initialWidth: SIDE_PANEL,
  });
  // The inspector and the transform are one panel with a pill between them, so there
  // is nothing to add for the second: they answer different questions about the same
  // selection and never wanted room of their own.
  api.addPanel({
    id: "inspector",
    component: "inspector",
    title: "Inspector",
    position: { referencePanel: "viewer", direction: "right" },
    initialWidth: SIDE_PANEL,
  });
}

function onReady(event: DockviewReadyEvent) {
  const { api } = event;
  holdDock(api);
  let restored = false;
  try {
    const saved = localStorage.getItem(LAYOUT_KEY);
    if (saved) {
      api.fromJSON(JSON.parse(saved));
      // Only the panels the default layout places. Checking every registered component
      // would make a saved layout stale the moment a panel exists that is opened on
      // demand, and throw away an arrangement somebody chose.
      restored = DEFAULT_PANELS.every((id) => api.getPanel(id));
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

// No panel-specific entries: the panels that had them are headless now, and both
// actions already live somewhere better. Importing is on the File menu, on the shortcut
// and on the empty bin's own button; adding a track is the plus in the timeline's
// toolbar. A panel that keeps its header can add entries here again.
const HeaderActions = makeHeaderActions(
  {},
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
