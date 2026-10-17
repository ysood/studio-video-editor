# ADR-0003: AVFoundation composition as the macOS preview engine

Status: Accepted
Date: 2026-10-17
Deciders: yuvraj

## Context

Milestone 1 previews the timeline with one muted HTML `<video>` per visual clip, loaded
through Tauri's asset protocol (`convertFileSrc`). On real footage this does not play.
Measured on 1920x1080 H.264 at 58 to 64 Mbps, 48 frame GOP:

- The machine is not the limit. `ffmpeg` decodes the same file at 13.6x real time in
  software and sustains about 120 fps through VideoToolbox for 0.15s of CPU.
- A release build changed nothing, so the Rust optimisation level is not the limit.
- `tauri-apps/tauri#7355` ("convertFileSrc can't serve large videos to frontend (macOS)")
  is open and describes the same symptom: high bitrate H.264 stutters and freezes on
  macOS only, suspected to be WKWebView's custom scheme implementation.

A webview `<video>` element also cannot composite tracks, so it is a dead end for the
effects, transforms and transitions that later milestones need.

## Options

1. Keep `<video>`, serve media from a localhost HTTP server with range support
   + Hours of work, likely restores playback  − Still cannot composite, still a stopgap
2. Adopt a cross platform editing engine (MLT, GStreamer GES, libopenshot)
   + Proven in Kdenlive, Shotcut, Pitivi  − Each owns a timeline model that collides with
   invariant 1, MLT's only Rust binding (`mlt-sys`) is untouched since 2018, GES documents
   itself as not thread safe
3. Decode ourselves and composite with wgpu
   + Full control, cross platform later  − We write and maintain a decoder, frame cache,
   playback clock and compositor before anything plays
4. AVFoundation: build an `AVMutableComposition` from the timeline and play it with
   `AVPlayer` into an `AVPlayerLayer` hosted in a native view
   + Apple's own editing engine, VideoToolbox backed, frame accurate, audio sync included
   − macOS only, custom effects later need `AVVideoCompositing` rather than our shaders

## Decision

Option 4 for the macOS preview. The composition is derived and preview only: the UI still
sends an `EditCommand`, `studio-core` still owns the document, and the composition is
rebuilt from the validated project afterwards. This mirrors how Kdenlive keeps its own
model and feeds MLT.

The picture is drawn by an `AVPlayerLayer` in an `NSView` added above the WKWebView and
positioned from the web UI, because the viewer's matte is a plain rectangle with no web
content over it. Nothing depends on making the webview transparent.

Render and export are unchanged: they stay with the FFmpeg argument vector in
`export::render`, so what you export is never what AVFoundation happened to show.

## Consequences

- The browser preview mode (`src/preview/`) keeps the `<video>` path, since it has no
  native shell. The two now differ, and the native path is the one that must be correct.
- Playback resolution rules live in two places again (`src/playback/resolve.ts` mirrors
  `render.rs`). The composition builder becomes a third. It is covered by tests that
  assert the composition's time ranges match what `resolve.rs` reports for the same frame.
- Windows and Linux keep the current preview until a second backend exists. The viewer is
  behind a trait so a wgpu or GStreamer backend can be added without touching the UI.
- Revisit if effects need per pixel control that `AVVideoCompositing` makes awkward, which
  is the point where the wgpu compositor in ADR-0001 earns its cost.
