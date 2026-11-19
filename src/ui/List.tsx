import { createElement, type CSSProperties, type HTMLAttributes, type ReactNode, type Ref } from "react";
import { cn } from "../lib/cn";

/**
 * Where a row says it is selected. "edge" is a stripe down the leading edge, which is
 * what a dense list wants: it costs no height and survives a row whose content already
 * fills it. "outline" draws around the whole thing, for a tile in a grid. "none" is for
 * a list whose rows are not a choice, such as the recent projects on the home screen.
 */
export type RowMarker = "edge" | "outline" | "none";

/**
 * A list of rows: no bullets, no browser indent, and the same breathing room at the ends
 * everywhere. Pass `label` when the rows are options the person picks between, which
 * makes the list a listbox; leave it off when each row is its own control.
 */
export function List({ label, layout, className, style, children }: {
  label?: string;
  /** "grid" lays the rows out as tiles. The caller sets the track sizes. */
  layout?: "grid";
  className?: string;
  style?: CSSProperties;
  children: ReactNode;
}) {
  return (
    <ul
      // No bullets, no browser indent. Tiles take the caller's track sizes, because only
      // the caller knows how wide a tile is.
      className={cn("m-0 list-none px-0 py-2", layout === "grid" && "grid gap-4", className)}
      data-layout={layout}
      role={label ? "listbox" : undefined}
      aria-label={label}
      style={style}
    >
      {children}
    </ul>
  );
}

/**
 * What a row owns: the states, and nothing about its layout. Its grid, padding and
 * radius belong to whatever kind of row it is.
 *
 * The fills are `state-*` rather than `bg-*`, so they layer over whatever resting fill
 * the row has instead of replacing it. Selected is listed after hover in the CSS the
 * variants compile to, so a selected row stays marked while the pointer is on it.
 *
 * Selection is read from `data-selected` and the marker from `data-marker`, not from
 * props, because the row may be a Radix `asChild` target that only passes attributes on.
 */
const rowStates = cn(
  "hover:state-hover focus-within:state-hover data-[selected=true]:state-selected",
  // The leading stripe is always there, transparent, so selecting a row never changes
  // its width.
  "data-[marker=edge]:border-l-(length:--border-focus-width) data-[marker=edge]:border-l-transparent",
  "data-[marker=edge]:data-[selected=true]:border-l-selection",
  "data-[marker=outline]:outline-solid data-[marker=outline]:outline-(length:--border-focus-width)",
  "data-[marker=outline]:outline-transparent data-[marker=outline]:-outline-offset-(--border-focus-width)",
  "data-[marker=outline]:transition-colors data-[marker=outline]:duration-quick",
  "data-[marker=outline]:ease-standard data-[marker=outline]:data-[selected=true]:outline-selection",
  "data-[tone=danger]:[&_svg]:text-danger",
);

/**
 * A selectable line in a list.
 *
 * It owns the states every list in the app has to agree on, the hover fill, the selected
 * fill and where selection is marked, and nothing about what sits inside: a bin folder,
 * a media row and an export choice genuinely differ there. Give the row its own grid in
 * your stylesheet and leave the states here.
 *
 * Passing `selected` is what makes a row a choice. A row without it is a plain line
 * that still highlights under the pointer.
 *
 * `element` is here because a choice is not always a line in a list: a radio wraps its
 * input in a label, and it should not have to restate these states to do that. Only a
 * list item can carry the `option` role, so the fills key off `data-selected`, which
 * every element can.
 */
export function Row({ element = "li", selected, marker = "edge", tone, className, children, ref, ...rest }: {
  element?: "li" | "label" | "div";
  selected?: boolean;
  marker?: RowMarker;
  tone?: "danger";
  className?: string;
  children: ReactNode;
  ref?: Ref<HTMLElement>;
} & Omit<HTMLAttributes<HTMLElement>, "className" | "children" | "role" | "aria-selected">) {
  return createElement(
    element,
    {
      ...rest,
      ref,
      className: cn(rowStates, className),
      "data-marker": marker,
      "data-tone": tone,
      "data-selected": selected,
      role: element === "li" && selected !== undefined ? "option" : undefined,
      "aria-selected": element === "li" ? selected : undefined,
    },
    children,
  );
}
