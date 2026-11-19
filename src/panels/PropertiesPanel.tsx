// The two panels that answer questions about the selection, as one card. They always
// shared a place in the layout; what they did not share was a way to switch that reads
// like the rest of the app.
//
// The switch is a tab strip rather than a control in a toolbar. These are two views of
// one panel, which is what a tab strip means everywhere else in software, and it is
// what Premiere and Resolve both use for the same job. The segmented control it
// replaced sat alone in the middle of an otherwise empty bar, which read as a cutout in
// the panel rather than as a way to change what the panel was showing.

import { useState } from "react";
import { Panel, PanelBody, Tabs } from "../ui";
import { InspectorBody } from "./InspectorPanel";
import { TransformBody } from "./TransformPanel";

type Properties = "inspector" | "transform";

export function PropertiesPanel() {
  const [showing, setShowing] = useState<Properties>("inspector");
  return (
    <Panel>
      <Tabs
        fill
        label="Properties"
        value={showing}
        onChange={(next) => setShowing(next as Properties)}
        tabs={[
          {
            value: "inspector",
            label: "Inspector",
            content: (
              <PanelBody layout="sheet">
                <InspectorBody />
              </PanelBody>
            ),
          },
          {
            value: "transform",
            label: "Transform",
            content: (
              <PanelBody layout="sheet">
                <TransformBody />
              </PanelBody>
            ),
          },
        ]}
      />
    </Panel>
  );
}
