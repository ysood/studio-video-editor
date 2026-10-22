import type { CSSProperties, LiHTMLAttributes, ReactNode, Ref } from "react";
import "./list.css";

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
      className={className ? `ui-list ${className}` : "ui-list"}
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
 * A selectable line in a list.
 *
 * It owns the states every list in the app has to agree on, the hover fill, the selected
 * fill and where selection is marked, and nothing about what sits inside: a bin folder,
 * a media row and an export choice genuinely differ there. Give the row its own grid in
 * your stylesheet and leave the states here.
 *
 * Passing `selected` is what makes a row a choice: it takes the `option` role and says
 * whether it is the chosen one. A row without it is a plain line that still highlights
 * under the pointer.
 */
export function Row({ selected, marker = "edge", tone, className, children, ref, ...rest }: {
  selected?: boolean;
  marker?: RowMarker;
  tone?: "danger";
  className?: string;
  children: ReactNode;
  ref?: Ref<HTMLLIElement>;
} & Omit<LiHTMLAttributes<HTMLLIElement>, "className" | "children" | "role" | "aria-selected">) {
  return (
    <li
      {...rest}
      ref={ref}
      className={className ? `ui-row ${className}` : "ui-row"}
      data-marker={marker}
      data-tone={tone}
      role={selected === undefined ? undefined : "option"}
      aria-selected={selected}
    >
      {children}
    </li>
  );
}
