# The agent API

Last verified: 2026-10-22

Studio is meant to be driven by an agent as well as by hand. This is how: a catalogue of
tools, a way of naming parts of the document in text, and a refusal shape that says what
to do instead. `crates/studio-agent` defines all three, and knows nothing about who is
calling or how they arrived.

## What a tool is

Every tool is one thing a caller can do, described well enough to decide whether to.

```rust
Tool {
    name: "timeline.move_clip",      // area.verb, so a list reads grouped
    area: Area::Timeline,
    title: "Move a clip",            // for a person reading a list
    description: ...,                // from the command's own doc comment
    input_schema: ...,               // JSON Schema, generated from the Rust type
    annotations: Annotations { read_only, destructive, idempotent, open_world },
    surfaces: [Surface::InApp, ...],
    in_default_surface: true,
}
```

The description is not written twice. `EditCommand`'s variants carry doc comments, the
`schema` feature of `studio-core` turns the enum into JSON Schema, and the per-variant
branch is pulled out with its description intact. A command whose behaviour changes and
whose comment is updated has told every caller.

**Annotations are the Model Context Protocol's hints, and its defaults are pessimistic:
unsaid, a tool is taken to be destructive and to reach an open world.** Every tool here
says. `destructive` reads as "content can be lost", not "changes something": a command
that sets a field you could read back first is not destructive, or every setter would be
and the hint would stop carrying information. The chat panel's approval prompt hangs off
exactly this bit.

**Surfaces** say where a tool may be offered. A pane inside the app is driving the
document its own window is showing, with somebody watching, so it may undo. A caller in
another process is not: an undo there rewinds whatever that person was doing, and nothing
in the protocol says whose edit is being undone.

**`in_default_surface`** is the curation. Most tools are not in it. A long list is the
usual way a caller gets worse rather than better, because every entry costs it attention
on every turn. `Policy { surface, also, except }` widens or narrows it by tool or by
area, which is what a settings page will eventually drive.

## Naming things without ids

Every id in the document is a uuid, which is right for the document and wrong for anyone
writing a command from a description. `studio-core`'s `select` turns the thought into
ids:

| What | Forms |
|---|---|
| Sequence | its id, or its name |
| Track | its id, `V1` / `a2`, or `video:1` (one based) |
| Time | a frame number, `00:00:12:00`, `start`, `end` |
| Clip | its id, `V1@00:00:12:00` (the clip covering that frame), `V1#3` (third along) |

The limits are the point. Every form resolves to exactly one thing or it is an error.
**Ambiguity is never guessed**: two tracks can end up sharing a name, so asking for that
name is refused rather than answered with whichever came first. Nothing is relative,
because "the next clip" needs an origin a selector does not have. And the playhead is not
addressable: where somebody is looking is window state, not part of the document
(ADR-0004), and it means nothing to a caller in another process. A caller that has a
playhead passes the frame.

`end` is why appending needs no command of its own: it is `InsertMedia` starting there.

## Being refused

A refusal is not an exception. It is the other half of the API, and it is written for
whoever reads it.

```rust
ToolError { code, message, remedy, retryable, revision, details }
```

| Code | Means |
|---|---|
| `unknownTool` | No tool by that name. |
| `notAvailable` | The tool exists but is not offered here. |
| `badArguments` | The arguments did not fit the schema. |
| `badSelector` | What was named could not be found, or named more than one thing. |
| `staleRevision` | The document has moved since the caller last looked. |
| `notFound` | What was named is not in the document. |
| `trackLocked`, `wrongTrackKind`, `mediaInUse`, `invalidArgument` | The document's own rules. |
| `invalid` | The edit would have left the document invalid. |
| `host` | Something outside the document refused: a file, a tool, a running job. |
| `busy` | Something else is using the one thing this needs. |

`retryable` is false for everything the document refuses, because applying a command is
deterministic: the same command against the same document is refused again. `busy` is the
only code worth retrying. A caller that cannot tell these apart spends its turns retrying
a decision that will not change.

`revision` is on every refusal, so a caller holding a stale id knows the difference
between "that never existed" and "the document moved, look again".

## Revisions

`Editor::revision()` is a number that changes whenever the document does, and does not
when a command changes nothing. Every edit tool takes an optional `expected_revision`:
left out, the edit applies to whatever the document is now, which is right for a caller
that has just read it and wrong for one acting on something it read a while ago. Given,
and different, the call is refused with `staleRevision` and nothing happens.

## What an edit did

`EditReport` says what a command created, which is what the interface needs to select the
new thing. It is not what a caller driving the document needs, because several commands
change clips they were never given: placing a clip clears the room it needs, moving one
does the same where it lands, removing a track takes its clips, and a link whose other
half has gone is dissolved.

So `delta(before, after)` compares the two documents rather than asking the command what
it meant to do. It reports clips created, removed and changed, with `Change::Moved`,
`Trimmed`, `Reslipped`, `Retracked`, `Renamed`, `Toggled`, `Transformed`, `Unlinked`,
`Linked`. `touched_beyond(named)` answers the question a caller actually has: did this
reach past what I asked about?

## The two callers

**The window**, through `agent_list_tools` and `agent_call_tool` in
`src-tauri/src/agent.rs`. An edit made this way goes through the same session, the same
validation and the same undo stack as one made by clicking, because it is the same
command. The only thing the IPC layer adds is a name to call it by and a `Host` for the
few tools that reach outside the document.

**The assistant**, through `src-tauri/src/chat/`, which calls the same `agent::run`.
See the "The assistant" section of `architecture.md`.

There is no MCP server yet. When there is, it is another adapter over the same catalogue:
nothing in `studio-agent` speaks a wire protocol.
