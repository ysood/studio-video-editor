/**
 * The names of every Tailwind utility Studio defines, as data.
 *
 * `src/theme/tailwind.css` declares what each name *means*; this file says which names
 * exist, because two consumers need the list in a form CSS cannot give them:
 *
 *   - `src/lib/cn.ts` hands it to tailwind-merge. Without it, tailwind-merge falls back
 *     to guessing from Tailwind's stock vocabulary, where `text-body` and `text-fg-muted`
 *     look like the same kind of class, and merging the two silently drops the colour.
 *     That failure is invisible: the class list looks right in the source and the colour
 *     is simply absent in the browser.
 *   - `scripts/check-design.mjs` uses the spacing names to tell a utility that resolves
 *     to a theme value from one that resolves to nothing at all.
 *
 * `npm run check:design` asserts that this file and the `@theme inline` block in
 * tailwind.css name exactly the same tokens, so the two cannot drift apart.
 */

/** Colours: every `bg-`, `text-`, `border-`, `ring-`, `fill-` and `stroke-` value. */
export const COLOR_TOKENS = [
  // Surfaces
  "base", "gutter", "panel", "raised", "field", "overlay", "locked",
  // Interaction states
  "hover", "selected",
  // Borders
  "edge", "edge-strong",
  // Text
  "fg", "fg-muted", "fg-faint", "fg-heading", "fg-on-accent", "fg-on-media",
  // Accent, and the things that are their own meaning
  "accent", "accent-hover", "accent-wash", "selection", "danger", "scrim",
  // Media kind
  "media-video", "media-video-edge",
  "media-audio", "media-audio-edge",
  "media-image", "media-image-edge",
  // shadcn/ui's own names, pointed at the roles above so a pasted component lands
  // wearing this product's clothes
  "background", "foreground", "card", "card-foreground", "popover", "popover-foreground",
  "primary", "primary-foreground", "secondary", "secondary-foreground",
  "muted", "muted-foreground", "accent-foreground",
  "destructive", "destructive-foreground", "border", "input", "ring",
] as const;

/** Type roles: the `text-<role>` size, weight, line height and tracking together. */
export const TEXT_ROLES = [
  "caption", "label", "body", "heading", "title", "timecode", "overline",
] as const;

/**
 * Spacing, which here also carries the fixed component dimensions so `h-control` and
 * `size-icon` reach them the way `p-4` reaches a spacing step.
 */
export const SPACING_TOKENS = [
  "hairline", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12",
  "control", "control-small", "control-large", "icon",
  "toolbar", "topbar", "statusbar", "tabbar", "panel-footer",
  "play-button", "swatch", "dot", "slider-track", "slider-thumb",
  "field-label", "field-number", "menu-min-width", "dialog-width",
  "progress-width", "progress-height",
] as const;

export const RADIUS_TOKENS = [
  "control", "button", "panel", "dialog", "clip", "round", "sm", "md", "lg",
] as const;

export const FONT_TOKENS = ["ui", "display", "numeric"] as const;
export const SHADOW_TOKENS = ["popover", "dialog"] as const;
