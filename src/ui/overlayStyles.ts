// The one anatomy every overlay in the app wears: menus, select lists, popovers and
// tooltips. They are different Radix primitives with the same job, so they share these
// class lists rather than four stylesheets that drift apart a line at a time.
//
// Overlays sit on the `well` surface, which is why each panel below also carries
// `data-surface="well"`: the surface restates the colour variables, so `bg-overlay`
// and `text-fg` resolve to the well's values inside one and the panel's everywhere else.

import { cn } from "../lib/cn";

/**
 * The floating panel itself.
 *
 * `ui-overlay` carries no style. It is the hook `playback/nativeViewer.ts` queries to
 * find open overlays that overlap the native preview surface, so it stays on the shared
 * anatomy and every overlay is found without that file listing four primitives.
 *
 * Entrance is `starting:` rather than a keyframe: Radix mounts the panel when it opens,
 * and the browser transitions it from the starting style, so no animation has to be
 * registered with the theme. The origin is the anchor Radix computes, so the panel grows
 * out of whatever opened it.
 */
export const overlayPanel = cn(
  "ui-overlay z-overlay overflow-hidden rounded-dialog border border-edge-strong bg-overlay p-1 text-fg shadow-popover",
  "origin-(--radix-popper-transform-origin) transition duration-quick ease-standard",
  "starting:scale-95 starting:opacity-0",
);

/** A menu panel, which unlike a tooltip holds rows and so needs a minimum width. */
export const overlayMenuPanel = cn(overlayPanel, "min-w-menu-min-width");

/**
 * A row inside one. Overlay rows take a highlight fill rather than a focus ring: the
 * keyboard and the pointer move the same highlight, and a ring on top of it would say
 * the same thing twice.
 */
export const overlayItem = cn(
  "relative flex h-control cursor-default select-none items-center gap-4 rounded-control px-4",
  "text-label text-fg outline-none",
  "data-[highlighted]:state-hover data-[state=checked]:text-accent",
  "data-[disabled]:pointer-events-none data-[disabled]:opacity-disabled",
  "[&_svg]:shrink-0",
);

/** A destructive row. It sits last, after a separator. */
export const overlayItemDanger = "text-danger data-[highlighted]:text-danger";

/** The hairline between groups of rows. */
export const overlaySeparator = "-mx-1 my-1 h-hairline bg-edge";

/** A shortcut, set in the numeric face and ranged right. */
export const overlayShortcut = "numeric ms-auto ps-6 text-caption text-fg-faint";
