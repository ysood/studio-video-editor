// Themed Dockview headers: our own tab (title only, no close button, since every panel
// is part of the editor) and a right-hand overflow menu per panel group. The header
// strip takes the surface of the active panel, so the timeline's header joins its dark
// stage while the rest of the frame stays chrome.

import type { IDockviewHeaderActionsProps, IDockviewPanelHeaderProps } from "dockview-react";
import { useEffect, useRef, useState } from "react";
import type { SurfaceName } from "../theme";
import { IconButton, Menu, type MenuEntry } from "../ui";
import "./panel-header.css";

/** Panels whose header should sit on a surface other than chrome. */
const PANEL_SURFACES: Record<string, SurfaceName> = {
  timeline: "stage",
};

/** Panel-specific entries shown above the shared ones in a panel's overflow menu. */
export type PanelMenus = Record<string, () => MenuEntry[]>;

export function PanelTab({ api }: IDockviewPanelHeaderProps) {
  const [title, setTitle] = useState(api.title ?? api.id);
  useEffect(() => {
    const subscription = api.onDidTitleChange((e) => setTitle(e.title));
    return () => subscription.dispose();
  }, [api]);
  return <div className="panel-tab">{title}</div>;
}

/** Builds the header actions component with the per-panel menus and shared actions in scope. */
export function makeHeaderActions(menus: PanelMenus, shared: () => MenuEntry[]) {
  return function HeaderActions({ activePanel, api }: IDockviewHeaderActionsProps) {
    const ref = useRef<HTMLDivElement>(null);
    const panelId = activePanel?.id ?? "";

    // Put the whole header strip on the active panel's surface.
    useEffect(() => {
      const strip = ref.current?.closest(".dv-tabs-and-actions-container");
      strip?.setAttribute("data-surface", PANEL_SURFACES[panelId] ?? "chrome");
    }, [panelId]);

    const own = menus[panelId]?.() ?? [];
    const maximized = api.isMaximized();
    const group: MenuEntry[] = [
      {
        label: maximized ? "Restore panel" : "Maximize panel",
        icon: "fit",
        onSelect: () => (maximized ? api.exitMaximized() : api.maximize()),
      },
      ...shared(),
    ];
    const entries: MenuEntry[] = own.length > 0 ? [...own, { type: "separator" }, ...group] : group;
    return (
      <div ref={ref} className="panel-header-actions">
        <Menu
          align="end"
          trigger={<IconButton icon="more" size="small" label={`${activePanel?.title ?? "Panel"} options`} />}
          entries={entries}
        />
      </div>
    );
  };
}
