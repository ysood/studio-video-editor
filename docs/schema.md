# Project schema

The project document is defined once, in Rust (`crates/studio-core/src/model.rs`).
TypeScript types in `src/bindings/` are generated from it by `ts-rs` and must never be
edited by hand. This page explains the shape and the invariants that
`Project::validate` enforces. When the code and this page disagree, the code is right and
this page is a bug.

Current `SCHEMA_VERSION`: **1**

## Shape

```
Project
├── schemaVersion, id, name
├── media: MediaAsset[]          (the bin)
│     id, name, path, kind (video | audio | image), durationUs,
│     video? { width, height, frameRate, codec },
│     audio? { sampleRate, channels, codec },
│     fingerprint { sizeBytes, modifiedUnixMs }
└── sequence: Sequence
      id, name,
      format { width, height, frameRate: Rational, sampleRate },
      tracks: Track[]
            id, kind (video | audio), name, enabled, locked,
            clips: Clip[]
                  id, name, start, duration, enabled, link?,
                  source: ClipSource
                        { type: "media", mediaId, sourceStart }
```

## Time

- `Frame` is an integer count of frames at `sequence.format.frameRate`. Every timeline
  `start`, `duration` and `sourceStart` uses it. There are no float times in the document.
- `Rational { num, den }` holds exact rates such as `24000/1001`.
- Media duration is stored as microseconds (`durationUs`) because that is what ffprobe
  reports. It converts to sequence frames with a floor, so a clip built from it never reads
  past the end of the file.
- Floats appear only when leaving the document: FFmpeg seek arguments and OTIO rates.

## Invariants

| Rule | Why |
|---|---|
| `schemaVersion` equals the build's `SCHEMA_VERSION` | Older files are migrated on load, never edited in an old shape |
| Every id is unique across project, sequence, media, tracks and clips | Commands address entities by id alone |
| Width and height are even, positive and at most 8192; sample rate is 44.1, 48 or 96 kHz; frame rate is positive and at most 240 | H.264 and ProRes 4:2:2 need even dimensions; these are the rates FFmpeg and NLEs share |
| Clips on a track are sorted by `start` and never overlap | Stacking is between tracks, never within one |
| `start >= 0`, `duration >= 1`, `sourceStart >= 0` | No negative time |
| A clip's media exists, and has a video stream on a video track or an audio stream on an audio track | Render and export can trust every reference |
| `sourceStart + duration` fits inside the media, except for still images | No reading past the end of a file |
| A link group has at least two clips on distinct tracks, all with equal `start`, `duration` and `source` | Linked video and audio stay in sync through every edit |
| Names are at most 256 characters | Bounded input from the UI and from project files |

## Track order

Tracks live in one list. Order within a kind is stacking order: a later video track
covers an earlier one. The UI shows video tracks top-down from highest to V1, then audio
tracks from A1 downward.

## Extending

- New clip kinds (generated, 3D, motion) are new `ClipSource` variants. Each must bake to a
  media file before export, so OTIO and XML stay media-only.
- Any change to a persisted type bumps `SCHEMA_VERSION` and adds a migration in
  `persist.rs` (see AGENTS.md invariant 4).
