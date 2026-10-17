# Architecture

Last verified: 2026-10-17

## Layers

```
┌────────────────────────────── React UI (src/) ──────────────────────────────┐
│  Workspace (Dockview)   Media bin   Viewer   Timeline (canvas)   Inspector   │
│  store.ts: latest Snapshot + view state (selection, playhead, zoom)          │
│  geometry.ts / resolve.ts: pure, tested previews that mirror core rules      │
└───────────────▲──────────────────────────────────────────────┬──────────────┘
                │ Snapshot / events                              │ EditCommand, file commands
┌───────────────┴──────────── Tauri shell (src-tauri/) ─────────▼──────────────┐
│  commands.rs: validate paths, call core, spawn ffprobe/ffmpeg, emit job events│
│  state.rs: Session (Editor + save path), running job, cached tool paths       │
└───────────────▲──────────────────────────────────────────────┬──────────────┘
                │                                                │
┌───────────────┴──────────── studio-core (crates/) ───────────▼──────────────┐
│  model.rs   schema          edit.rs     commands, overwrite, links           │
│  validate.rs invariants     history.rs  undo/redo snapshots, dirty tracking  │
│  persist.rs versioned JSON  probe.rs    ffprobe JSON to MediaAsset           │
│  export/    otio.rs  fcp7.rs  render.rs (FFmpeg plan)  package.rs (conform)  │
└──────────────────────────────────────────────────────────────────────────────┘
```

## Rules that hold the design together

1. **One owner.** Only `studio-core` changes a `Project`. The UI sends an `EditCommand` and
   renders the returned `Snapshot`. A future agent will call the same commands, so every
   agent action is undoable and validated by construction.
2. **One time unit.** Integer frames at the sequence rate everywhere in the document
   (`docs/schema.md`). Floats exist only in FFmpeg arguments and OTIO rates.
3. **One schema definition.** TypeScript types are generated from Rust by `ts-rs` into
   `src/bindings/`; `npm run check:bindings` fails on drift.
4. **Apply to a copy, validate, swap.** `edit::apply` never mutates in place, so a rejected
   command leaves nothing half-done, and undo is a stack of whole snapshots.
5. **Previews mirror, core decides.** Drag previews (`src/timeline/geometry.ts`) and
   playback resolution (`src/playback/resolve.ts`) replicate core rules for instant
   feedback. If they disagree, the core result wins on drop.

## Data flow of one edit

1. Pointer up on a trimmed clip edge.
2. `TimelinePanel` sends `{ type: "TrimClip", clipId, edge, frame }` through `ipc.edit`.
3. `commands::edit` locks the session and calls `Editor::apply`.
4. `edit::apply` clones the project, clamps and applies the trim to the link group,
   normalizes links, runs `Project::validate`, and returns the new project.
5. The editor pushes the old project onto the undo stack and returns a fresh `Snapshot`.
6. The store replaces its snapshot and prunes selection; panels re-render.

## App actions and menus

New, open, save, import, export, undo, redo and reset layout are `MenuAction`s, a Rust
enum exported to TypeScript. Every way of starting one ends in `runAction` in
`src/actions.ts`: the native menu, the in-window File menu, buttons and shortcuts.

- **macOS:** `src-tauri/src/menu.rs` installs the system menu bar (Studio, File, Edit,
  View, Window). Choosing an item emits `menu://action` and the UI runs the action. Each
  command that changes the session calls `menu::refresh` after releasing the session
  lock, so the Edit menu names the step to undo ("Undo Move clip") and Export is enabled
  only when the timeline has clips. Cut, copy, paste and select all are native items so
  text fields behave normally; Cmd+Z in a focused text field is passed back to that field.
- **Elsewhere:** the top bar draws a File menu instead, since a native menu there is a
  second bar inside the window. `STUDIO_NATIVE_MENU=1` forces the native menu for testing.
- A shortcut can reach both the web view and the native menu, so a menu event arriving
  within 300 ms of the keyboard running the same action is ignored.

## Media, playback and render

- **Import:** `ffprobe` (found via `STUDIO_FFPROBE`, PATH, or Homebrew paths) runs per file
  in parallel batches of four; output is parsed by `probe::parse`. Imported paths are added
  to the asset protocol scope, which starts empty.
- **Playback:** the webview's decoders (AVFoundation on macOS) play original media through
  the asset protocol. The highest enabled video track wins; every enabled audio clip is
  heard. Elements are re-seeked only beyond 120 ms of drift.
- **Render:** `export::render::plan` builds an FFmpeg argument vector (black canvas plus
  silent bed, one seeked input per clip, fit, offset, overlay in track order, delayed and
  mixed audio). The shell spawns it, streams `-progress` to `job://progress`, and deletes
  partial output on failure or cancel.

## Handoff to Resolve and Premiere

See ADR-0002. OTIO JSON is canonical; FCP7 XML is the Premiere path; a conform package
copies used media next to both files plus a provenance manifest.

## Adding a clip type (generated, 3D, motion)

1. Add a `ClipSource` variant in `model.rs` with whatever it needs (provider, prompt, seed,
   take list, active take). Bump `SCHEMA_VERSION` and add a migration in `persist.rs`.
2. Extend `validate.rs` (for example: the active take must exist).
3. Give it a baked media representation (the active take's file) so `render.rs`, `otio.rs`,
   `fcp7.rs` and `package.rs` keep treating every clip as media.
4. Add commands (`GenerateTake`, `SetActiveTake`) and their tests in `edit.rs`; long jobs
   go through the shell's job mechanism like renders.
5. Regenerate bindings, then add UI.

## Known limits (milestone 1)

- Preview fidelity is whatever the webview decodes: no effects, transforms or transitions
  yet. In a headless Linux WebKitGTK session video frames did not render (stills did); on
  macOS the system webview plays H.264, HEVC and ProRes. Not yet verified on a Mac.
- No speed changes, ripple edits, transitions, markers, keyframes or audio levels.
- FCP7 XML writes `in` and `out` at the sequence rate; mixed-rate footage may need conform.
- FFmpeg must be installed; bundling it as a Tauri sidecar is planned.
- Quitting (Cmd+Q) or closing the window does not yet ask about unsaved changes.
- The native menu was tested on Linux (GTK) with `STUDIO_NATIVE_MENU=1`, not yet on a Mac.
- Files dragged in from Finder are not accepted yet (use Import); HTML drag and drop is
  used for bin to timeline.
