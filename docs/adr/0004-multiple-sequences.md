# ADR-0004: Several sequences in a project, addressed by id

Status: Accepted
Date: 2026-10-17
Deciders: yuvraj

## Context

`Project` holds exactly one `Sequence`, created by `Project::new` and never added to.
There is no way to make a second one, and the bin lists media only. Every editor this
tool hands off to assumes otherwise: in Premiere, sequences sit in the Project panel
beside the clips, and in Resolve, timelines sit in the Media Pool and are configured by
right-clicking them there. A project with one deliverable per timeline, or a rough cut
beside a final, is ordinary work.

The single sequence is also why the frame rate locks once the timeline has clips. With
more than one sequence, the answer to footage that does not match is a new sequence that
does, which is Premiere's "new sequence from clip".

`.sequence` is read in 65 places across 12 Rust files and 16 TypeScript files, including
all four export paths, so this reaches most of the codebase even though each change is
small.

## Options

1. One sequence, as today
   + Nothing to do  − Does not match any editor anyone has used; the frame rate trap stays
2. Several sequences, with the core tracking which one is "current"
   + Commands keep their present shape  − The core gains hidden global state: a command's
   meaning depends on an earlier command. Undo, agents and any second window all have to
   agree on it, and switching sequences would push an undo entry and mark the file dirty
3. Several sequences, with every timeline command naming its target
   + No hidden state: a command means the same thing wherever it is issued, which is what
   lets an agent or a second window drive the document safely
   − Touches the signature of nearly every command and every export entry point

## Decision

Option 3.

`Project.sequences: Vec<Sequence>` replaces `Project.sequence`, and every command that
touches a timeline carries a `sequence_id`. Nothing in the document records which
sequence is open, because that is a property of a window, not of the work.

Which sequence is open, and the playhead and zoom for each, are UI state keyed by
sequence id, restored from local preferences the way the media bin's own preferences
already are. Premiere does the same thing, down to it being a preference
("Restore open sequences when opening projects") rather than project data. Its
implementation is widely reported to restore the wrong tabs, so restoring one sequence
well is worth more than restoring a set of tabs badly.

Sequences appear in the bin as a "Timelines" entry beside the existing smart bins, with a
button to add one. This is Resolve's model and needs no concept the user does not already
have from Footage, Audio and Stills.

Interchange keeps exporting one sequence per file. OTIO can hold several in a
`SerializableCollection`, but its own documentation warns that most tools expect a single
timeline per file, and a handoff that Resolve silently half-reads is worse than two files.

Nesting is out of scope. A `ClipSource::Sequence` variant would break the rule in ADR-0002
that every clip bakes to a media file before export, so nesting needs a render-to-media
step that does not exist yet.

## Consequences

- `SCHEMA_VERSION` goes to 2 with a migration that wraps the single `sequence` in a list.
  `docs/schema.md` changes in the same commit.
- `Project::validate` gains: at least one sequence, sequence ids unique, and every
  command's target resolvable. Ids were already required to be unique project-wide.
- `otio::export`, `fcp7::export`, `render::plan` and `package::plan` take the sequence to
  export rather than reaching for the only one. The export dialog has to say which.
- A deleted sequence cannot be the last one. Until nesting exists, nothing else can refer
  to a sequence, so deletion has no other consequence.
- Per-sequence view state means the store keys playhead, zoom and scroll by sequence id
  instead of holding one of each.
- The frame rate stays locked per sequence once that sequence has clips. That restriction
  becomes tolerable rather than a trap, because making another sequence is now possible.
- Build it in the repository's usual order: schema and migration, then command targeting,
  then exports, then IPC, then the bin and the new sequence dialog. Each layer is its own
  commit and green on its own.
