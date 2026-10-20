# Architecture

Last verified: 2026-10-20

## Layers

```
┌────────────────────────────── React UI (src/) ──────────────────────────────┐
│  Workspace (Dockview)  Media bin  Viewer  Timeline  Inspector + Transform    │
│  store.ts: latest Snapshot + view state (selection, playhead, zoom)          │
│  geometry.ts / resolve.ts / framing.ts: pure, tested mirrors of core rules   │
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
- **Playback (macOS):** `viewer.rs` flattens the timeline into segments, builds an
  `AVMutableComposition` from them and plays it with `AVPlayer` into an `AVPlayerLayer`,
  in a native view positioned over the viewer's matte by the web UI (ADR-0003). The
  highest enabled video track wins, matching `render.rs`; audio keeps one lane per track.
  The asset protocol is not on this path: it stalls on high bitrate H.264 (tauri#7355).
  AVFoundation is also the clock: a periodic time observer reports the player's own time
  and the UI sets the playhead from it, so the timecode always matches the picture.
- **Playback (elsewhere, and browser preview):** one muted `<video>` per visual clip
  through the asset protocol, re-seeked only beyond 120 ms of drift, with the playhead
  advanced from wall time because there is no player to follow.
- **Render:** `export::render::plan` builds an FFmpeg argument vector (black canvas plus
  silent bed, one seeked input per clip, fit, transform, offset, overlay in track order,
  delayed and mixed audio). The shell spawns it, streams `-progress` to `job://progress`,
  and deletes partial output on failure or cancel.
- **Compositing:** a clip is overlaid at its own size, so whatever it does not cover shows
  the layer below (ADR-0005). Each clip is fitted to the frame first and its `Transform`
  applied in that space, which keeps an identity transform rendering what it always did.

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

- Clips carry a transform (position, scale, rotation, anchor, opacity) and the stack
  composites, in the renderer and both previews. No other effects and no transitions yet,
  and no keyframes: a transform holds for the whole clip. On macOS the picture comes
  from AVFoundation and plays 60 Mbps H.264 smoothly; the element path used everywhere
  else still cannot stream that bitrate through the asset protocol.
- Transforms travel in the handoff, but have not been round-tripped through Premiere or
  Resolve on a real machine. FCP7 XML writes `Basic Motion` and `Opacity`; OTIO carries
  the transform in `metadata.studio` because it has no schema for one. The unit
  conventions are taken from a Premiere export, not from a spec (ADR-0005), so the first
  real import is worth checking by eye. One transform per clip travels, not keyframes.
- `Basic Motion` holds one Scale, so a clip scaled differently across and down reaches
  Premiere as its width scale and the other is lost. Nothing in the format holds it.
- A clip can be dragged and scaled in the viewer, but not turned: rotation and the anchor
  stay in the Transform panel. During a drag only the outline moves and the picture
  follows on release, because showing it live would mean rebuilding the video composition
  per pointer move (ADR-0006).
- The handle box is the picture's fitted rectangle, worked out from the media's stored
  dimensions. Footage a camera recorded sideways reports the size it is stored at, so the
  box can sit wrong on that clip even though the picture is drawn correctly; the renderer
  and both previews get the real size from the decoder, and this does not.
- The playhead follows the player at 30 Hz, so on sequences faster than 30 fps the
  reported frame skips one between updates. Raise `OBSERVE_HZ` if that shows.
- Still images are skipped by the macOS preview; only clips with a decodable video or
  audio track reach the composition.
- No speed changes, ripple edits, transitions, markers, keyframes or audio levels.
- A flip is not a negative scale: scale stays positive because FFmpeg's `scale` filter
  needs positive dimensions. A flip will be its own effect.
- FCP7 XML writes `in` and `out` at the sequence rate; mixed-rate footage may need conform.
- FFmpeg must be installed; bundling it as a Tauri sidecar is planned.
- Quitting (Cmd+Q) or closing the window does not yet ask about unsaved changes.
- The native menu is verified on macOS; `STUDIO_NATIVE_MENU=1` forces it elsewhere for testing.
- Files dragged in from Finder are not accepted yet (use Import); HTML drag and drop is
  used for bin to timeline.
