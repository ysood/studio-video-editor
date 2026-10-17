// Menus on Radix (keyboard navigation, typeahead, focus return, collision handling),
// described as data so every menu in the app has the same anatomy: optional icon,
// label, optional shortcut, separators and section labels.

import * as ContextMenuPrimitive from "@radix-ui/react-context-menu";
import * as DropdownMenuPrimitive from "@radix-ui/react-dropdown-menu";
import type { ReactNode } from "react";
import { Icon, type IconName } from "./Icon";
import { useOverlayContainer, useOverlayOpen } from "./overlay";
import "./overlay.css";

export type MenuEntry =
  | {
      type?: "item";
      label: string;
      onSelect: () => void;
      icon?: IconName;
      shortcut?: string;
      disabled?: boolean;
      tone?: "danger";
    }
  | { type: "separator" }
  | { type: "label"; label: string };

type Parts = typeof DropdownMenuPrimitive | typeof ContextMenuPrimitive;

function Entries({ entries, parts }: { entries: MenuEntry[]; parts: Parts }) {
  return (
    <>
      {entries.map((entry, index) => {
        if (entry.type === "separator") return <parts.Separator key={index} className="ui-menu-separator" />;
        if (entry.type === "label")
          return (
            <parts.Label key={index} className="ui-menu-label">
              {entry.label}
            </parts.Label>
          );
        return (
          <parts.Item
            key={index}
            className="ui-menu-item"
            data-tone={entry.tone}
            disabled={entry.disabled}
            onSelect={entry.onSelect}
          >
            <span className="ui-menu-icon">{entry.icon && <Icon name={entry.icon} />}</span>
            <span className="ui-menu-text">{entry.label}</span>
            {entry.shortcut && <span className="ui-menu-shortcut numeric">{entry.shortcut}</span>}
          </parts.Item>
        );
      })}
    </>
  );
}

/** A dropdown menu opened by `trigger` (usually a Button or IconButton). */
export function Menu({ trigger, entries, align = "start" }: {
  trigger: ReactNode;
  entries: MenuEntry[];
  align?: "start" | "center" | "end";
}) {
  const container = useOverlayContainer();
  const [open, setOpen] = useOverlayOpen();
  return (
    <DropdownMenuPrimitive.Root open={open} onOpenChange={setOpen}>
      <DropdownMenuPrimitive.Trigger asChild>{trigger}</DropdownMenuPrimitive.Trigger>
      <DropdownMenuPrimitive.Portal container={container}>
        <DropdownMenuPrimitive.Content className="ui-overlay ui-menu" data-surface="well" align={align} sideOffset={4}>
          <Entries entries={entries} parts={DropdownMenuPrimitive} />
        </DropdownMenuPrimitive.Content>
      </DropdownMenuPrimitive.Portal>
    </DropdownMenuPrimitive.Root>
  );
}

/** Wraps `children` so a right click (or Control-click) opens a menu. */
export function ContextMenu({ children, entries }: { children: ReactNode; entries: MenuEntry[] }) {
  const container = useOverlayContainer();
  // Left uncontrolled: Radix places a context menu from the event that opened it, so it
  // only needs telling when the state changes.
  const [, setOpen] = useOverlayOpen();
  return (
    <ContextMenuPrimitive.Root onOpenChange={setOpen}>
      <ContextMenuPrimitive.Trigger asChild>{children}</ContextMenuPrimitive.Trigger>
      <ContextMenuPrimitive.Portal container={container}>
        <ContextMenuPrimitive.Content className="ui-overlay ui-menu" data-surface="well">
          <Entries entries={entries} parts={ContextMenuPrimitive} />
        </ContextMenuPrimitive.Content>
      </ContextMenuPrimitive.Portal>
    </ContextMenuPrimitive.Root>
  );
}
