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

## Prerequisites (macOS)

```sh
xcode-select --install                                           # compilers
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Rust
brew install node ffmpeg                                         # Node 22+, FFmpeg
```

Studio finds `ffmpeg` and `ffprobe` on PATH or in `/opt/homebrew/bin`. Set
`STUDIO_FFMPEG` / `STUDIO_FFPROBE` to override.

## Run

```sh
npm install
npm run app          # tauri dev: Vite on :1420 plus the Rust shell
```

## Test

```sh
cargo test --workspace          # core, including a real FFmpeg render when installed
npm run check                   # typecheck, UI tests, bindings drift check
```

## Layout

| Path | What lives there |
|---|---|
| `crates/studio-core` | Project schema, edit commands, undo, persistence, probe parsing, render plan, OTIO / FCP7 XML / conform package export |
| `src-tauri` | Tauri shell: IPC commands, FFmpeg process management, asset scope |
| `src/bindings` | TypeScript types generated from Rust by ts-rs (do not edit) |
| `src/ipc` | Typed wrappers over the IPC commands |
| `docs/` | Architecture, schema reference, ADRs |

## Docs

- `docs/architecture.md`: layers, data flow, known limits, how to add a clip type
- `docs/schema.md`: the project document and its invariants
- `docs/adr/`: decisions and why they were made
- `AGENTS.md`: rules for coding agents, including commit identity
