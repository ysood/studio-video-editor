# ADR-0001: Tauri 2 shell, Rust core that owns the document, React UI

Status: Accepted
Date: 2026-10-17
Deciders: yuvraj

## Context

Studio is a desktop editor that must cut multi-track timelines, render them, and hand
timelines to Resolve and Premiere for color and finishing. Later milestones add generated
(AI, hosted API such as Seedance) and 3D clip types, plus an agent that edits the
timeline. The owner knows TypeScript and React well and wants Rust from the start rather
than a later port. Finishing (grading, advanced color) is out of scope by design.

## Options

1. Electron + TypeScript everywhere
   + One language, fastest UI iteration  − No natural home for media I/O and rendering;
   a later Rust port would cross every boundary at once
2. Tauri 2 + Rust core + React UI
   + Typed, testable core shared by UI, CLI and future agent; small binaries; system
   webview plays H.264, HEVC and ProRes on macOS  − Two languages; WebKit quirks on macOS;
   IPC boundary to design
3. Fully native Rust UI (egui, Slint, GPUI)
   + Fastest runtime, one language  − Slow UI iteration; few ready components for docking,
   inspectors and forms

## Decision

Option 2. The Rust crate `studio-core` owns the project schema, every edit command,
validation, undo history, persistence and export. The Tauri crate is a thin adapter that
spawns FFmpeg and exposes commands. React renders snapshots and sends commands; it never
mutates the document. TypeScript types are generated from Rust with `ts-rs` so the schema
has one definition.

Rendering and probing use the system FFmpeg in this milestone. Preview uses HTML media
elements in the webview, which is enough for cuts and stacking but not for effects.

## Consequences

- Every new edit needs a Rust command and a test before the UI can use it.
- Snapshot-per-edit IPC is simple and correct at current project sizes; switch to patches
  if a snapshot exceeds 1 MB or edits take more than 16 ms round trip.
- FFmpeg must be installed (Homebrew) until it is bundled as a sidecar.
- Preview fidelity is limited to what the webview decodes. Revisit (new ADR) when effects,
  generated layers or 3D need GPU compositing: the planned path is a wgpu renderer in Rust.
