import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

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
