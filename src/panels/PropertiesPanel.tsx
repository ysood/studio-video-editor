// The two panels that answer questions about the selection, as one panel. The switch
// between them is the same centred control the bin uses to choose a bin, because it is
// the same question: which view of this panel am I looking at.

import { useState } from "react";
import { Panel, PanelBody, PanelToolbar, Segmented } from "../ui";
import { InspectorBody } from "./InspectorPanel";
import { TransformBody } from "./TransformPanel";

type Properties = "inspector" | "transform";

export function PropertiesPanel() {
  const [showing, setShowing] = useState<Properties>("inspector");
  return (
    <Panel>
      <PanelToolbar>
        <Segmented
          align="center"
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
