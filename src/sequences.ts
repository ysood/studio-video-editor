// Which sequence a view is showing.
//
// A project holds a list of sequences (ADR-0004). Which one is on screen is a property
// of the window, not of the work, so it lives in the UI store and never in the
// document: switching timelines is not an edit and must not mark the project dirty.

import type { Project } from "./bindings/Project";
import type { Sequence } from "./bindings/Sequence";

/**
 * The sequence a view should show. Falls back to the first when nothing is chosen, or
 * when the chosen one has gone: a project always has at least one, so this cannot fail
 * on a validated document.
 */
export function activeSequence(project: Project, id: string | null): Sequence {
  const chosen = id === null ? undefined : project.sequences.find((s) => s.id === id);
  const sequence = chosen ?? project.sequences[0];
  if (!sequence) throw new Error("a project always has at least one sequence");
  return sequence;
}
