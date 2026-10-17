# Studio

A desktop video editor: import media, cut a multi-track timeline, play it back, render it
with FFmpeg, and hand the timeline to DaVinci Resolve or Premiere Pro for finishing.
Generated (AI) and 3D clip types are next.

Built with Tauri 2, a Rust core (`crates/studio-core`) that owns the project document, and
a React UI.

## What works (milestone 1)

- Import video, audio and stills (probed with ffprobe); offline media is flagged
- Multi-track timeline: drag from the bin, overwrite placement, move between tracks,
  trim with clamping, split, delete, snapping, linked video and audio, track
  hide/mute/lock, unlimited undo and redo
- Viewer playback with transport and keyboard control (Space to play or pause, K to
  stop, L to play, arrows to step, Home and End)
- Inspector for clip timing, source details and sequence settings
- Export: H.264 MP4 or ProRes 422 HQ render with progress and cancel, FCP7 XML,
  OpenTimelineIO, and conform packages for Resolve or Premiere handoff
- Projects saved as versioned `.studio` JSON with atomic writes

## Run (macOS)

Everything installs inside this folder. Only Apple's command line tools are needed
system-wide (`xcode-select --install`, most Macs already have them).

```sh
./scripts/dev.sh
```

The first run downloads Node and Rust into `.toolchain/` and FFmpeg into `node_modules/`
(about 1 GB with the first build), then opens the app. After that it starts in seconds.
Leave it running: UI changes appear instantly, and Rust changes rebuild and relaunch the
app on their own.

Other modes:

```sh
./scripts/dev.sh --browser   # UI only, in a browser at localhost:1420 with sample data
./scripts/dev.sh --setup     # install the toolchain without starting the app
```

To remove everything it installed, delete `.toolchain/`, `node_modules/` and `target/`.
Set `STUDIO_FFMPEG` / `STUDIO_FFPROBE` to use a different FFmpeg.

## Test

```sh
cargo test --workspace          # core, including a real FFmpeg render when installed
npm run check                   # typecheck, UI tests, bindings drift, design token guard
```

## Layout

| Path | What lives there |
|---|---|
| `crates/studio-core` | Project schema, edit commands, undo, persistence, probe parsing, render plan, OTIO / FCP7 XML / conform package export |
| `src-tauri` | Tauri shell: IPC commands, FFmpeg process management, asset scope |
| `src/bindings` | TypeScript types generated from Rust by ts-rs (do not edit) |
| `src/ipc` | Typed wrappers over the IPC commands |
| `src/theme` | Themes and design tokens: the only place visual values live |
| `src/ui` | UI primitives (buttons, fields, panels, icons, dialog, timecode) |
| `src/preview` | Sample backend used when the UI runs in a plain browser |
| `docs/` | Architecture, schema reference, ADRs |

## Docs

- `docs/architecture.md`: layers, data flow, known limits, how to add a clip type
- `docs/design-system.md`: themes, tokens, UI primitives, the design gallery
- `docs/schema.md`: the project document and its invariants
- `docs/adr/`: decisions and why they were made
- `AGENTS.md`: rules for coding agents, including commit identity
