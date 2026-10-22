/**
 * Stacking order for the whole app, so a new layer is a named step rather than a number
 * someone picked and nobody dares change.
 *
 * It is not part of a theme: how things stack is a fact about the interface, not a look,
 * and a theme that could reorder it would only be a way to break it.
 *
 * Dialogs are absent on purpose. They are real `<dialog>` elements, so the browser puts
 * them and their backdrop in the top layer, above every z-index on the page. Nothing
 * here can cover one, which is the point.
 */
export const layers = {
  /** Lifts an element over the background of the one after it, as a focus ring must. */
  raised: 1,
  /** A toolbar or ruler that stays put while the content under it scrolls. */
  sticky: 10,
  /** Menus, select lists, popovers, tooltips. */
  overlay: 100,
} as const;

export type LayerName = keyof typeof layers;
