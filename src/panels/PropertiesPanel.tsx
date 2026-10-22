// The two panels that answer questions about the selection, as one card. They always
// shared a place in the layout; what they did not share was a way to switch that reads
// like the rest of the app, because the tabs were the dock's rather than ours.

import { useState } from "react";
import { Panel, PanelBody, PanelToolbar, Segmented } from "../ui";
import { InspectorBody } from "./InspectorPanel";
import { TransformBody } from "./TransformPanel";

type Properties = "inspector" | "transform";

export function PropertiesPanel() {
  const [showing, setShowing] = useState<Properties>("inspector");
  return (
    <Panel surface="stage">
      <PanelToolbar>
        <Segmented
          emphasis="accent"
          label="Properties"
          value={showing}
          onChange={(next) => setShowing(next as Properties)}
          options={[
            { value: "inspector", label: "Inspector" },
            { value: "transform", label: "Transform" },
          ]}
        />
      </PanelToolbar>
      <PanelBody layout="sheet">
        {showing === "inspector" ? <InspectorBody /> : <TransformBody />}
      </PanelBody>
    </Panel>
  );
}
