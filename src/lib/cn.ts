import { clsx, type ClassValue } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";
import {
  COLOR_TOKENS,
  FONT_TOKENS,
  RADIUS_TOKENS,
  SHADOW_TOKENS,
  SPACING_TOKENS,
  TEXT_ROLES,
} from "../theme/vocabulary";

/**
 * tailwind-merge, taught Studio's vocabulary.
 *
 * It has to be told. Out of the box it recognises Tailwind's stock scale, so it reads
 * `text-body` and `text-fg-muted` as two classes of the same kind and keeps only the
 * last one, which quietly deletes the colour from every control that sets a type role
 * and a colour together. Nothing fails: the source still lists both classes, and the
 * browser simply never gets the colour. Handing it the real token names is what makes
 * `cn("text-fg-muted", "text-body")` keep both and `cn("text-fg-muted", "text-fg")`
 * keep the second.
 *
 * `src/theme/vocabulary.ts` is the list, and `npm run check:design` holds it to what
 * `src/theme/tailwind.css` actually defines.
 */
const twMerge = extendTailwindMerge({
  override: {
    theme: {
      color: [...COLOR_TOKENS],
      text: [...TEXT_ROLES],
      spacing: [...SPACING_TOKENS],
      radius: [...RADIUS_TOKENS],
      font: [...FONT_TOKENS],
      shadow: [...SHADOW_TOKENS],
    },
  },
});

/**
 * Join class names, letting a later one win over an earlier one that sets the same
 * property. Every component in `src/ui` takes a `className` so a caller can adjust it
 * at the point of use; without the merge, `<Button className="h-toolbar">` would land
 * two heights in the list and the winner would depend on source order in the stylesheet
 * rather than on what the caller asked for.
 *
 * This is shadcn/ui's `cn`, and shadcn components are pasted in expecting it by name.
 */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
