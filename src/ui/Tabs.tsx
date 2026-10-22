import * as TabsPrimitive from "@radix-ui/react-tabs";
import * as ToggleGroupPrimitive from "@radix-ui/react-toggle-group";
import type { ReactNode } from "react";
import { Icon, type IconName } from "./Icon";
import { Tooltip } from "./Tooltip";
import "./tabs.css";

export interface TabSpec {
  value: string;
  label: string;
  content: ReactNode;
}

/** Text tabs with arrow-key navigation; the selected tab's content renders below. */
export function Tabs({ value, onChange, tabs, label }: {
  value: string;
  onChange: (value: string) => void;
  tabs: TabSpec[];
  label: string;
}) {
  return (
    <TabsPrimitive.Root className="ui-tabs" value={value} onValueChange={onChange}>
      <TabsPrimitive.List className="ui-tab-list" aria-label={label}>
        {tabs.map((t) => (
          <TabsPrimitive.Trigger key={t.value} value={t.value} className="ui-tab">
            {t.label}
          </TabsPrimitive.Trigger>
        ))}
      </TabsPrimitive.List>
      {tabs.map((t) => (
        <TabsPrimitive.Content key={t.value} value={t.value} className="ui-tab-content">
          {t.content}
        </TabsPrimitive.Content>
      ))}
    </TabsPrimitive.Root>
  );
}

export interface SegmentOption {
  value: string;
  label: string;
  icon?: IconName;
  /** Show the icon alone, with the label as its tooltip and its accessible name. */
  iconOnly?: boolean;
}

/**
 * A segmented control: pick exactly one of a few options. An option may carry an icon,
 * a label, or both; `iconOnly` drops the text and moves the label into a tooltip, which
 * is what a dense toolbar wants.
 */
export function Segmented({ value, options, onChange, label, emphasis = "quiet", className }: {
  value: string;
  options: SegmentOption[];
  onChange: (value: string) => void;
  label: string;
  className?: string;
  /**
   * "accent" fills the chosen segment, for the control that is a panel's main choice.
   * "quiet" lifts it instead, for a secondary one such as a view toggle. Two accented
   * groups in one toolbar would both claim to be the main thing.
   */
  emphasis?: "quiet" | "accent";
}) {
  return (
    <ToggleGroupPrimitive.Root
      type="single"
      className={className ? `ui-segmented ${className}` : "ui-segmented"}
      data-emphasis={emphasis}
      value={value}
      aria-label={label}
      onValueChange={(next) => next && onChange(next)}
    >
      {options.map((o) => {
        const item = (
          <ToggleGroupPrimitive.Item
            key={o.value}
            value={o.value}
            className="ui-segment"
            data-icon-only={o.iconOnly ? "" : undefined}
            aria-label={o.label}
          >
            {o.icon && <Icon name={o.icon} />}
            {!o.iconOnly && o.label}
          </ToggleGroupPrimitive.Item>
        );
        return o.iconOnly ? (
          <Tooltip key={o.value} label={o.label}>
            {item}
          </Tooltip>
        ) : (
          item
        );
      })}
    </ToggleGroupPrimitive.Root>
  );
}
