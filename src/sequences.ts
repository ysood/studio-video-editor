// Reaching the sequence a view is showing.
//
// A project holds a list of sequences (ADR-0004). Until the UI tracks which one is
// open, every view means the first. This is the one place that assumption lives, so
// replacing it later is a change to this file and its callers, not a hunt.

import type { Project } from "./bindings/Project";
import type { Sequence } from "./bindings/Sequence";

/** The sequence to show when nothing has said which. Validation guarantees one exists. */
export function firstSequence(project: Project): Sequence {
  const sequence = project.sequences[0];
  if (!sequence) throw new Error("a project always has at least one sequence");
  return sequence;
}
