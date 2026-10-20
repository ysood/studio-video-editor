# Project schema

The project document is defined once, in Rust (`crates/studio-core/src/model.rs`).
TypeScript types in `src/bindings/` are generated from it by `ts-rs` and must never be
edited by hand. This page explains the shape and the invariants that
`Project::validate` enforces. When the code and this page disagree, the code is right and
this page is a bug.

Current `SCHEMA_VERSION`: **3**

## Shape

```
Project
├── schemaVersion, id, name
├── media: MediaAsset[]          (the bin)
│     id, name, path, kind (video | audio | image), durationUs,
│     video? { width, height, frameRate, codec },
│     audio? { sampleRate, channels, codec },
│     fingerprint { sizeBytes, modifiedUnixMs }
└── sequences: Sequence[]      (at least one; shown in the bin)
      id, name,
      format { width, height, frameRate: Rational, sampleRate },
      tracks: Track[]
            id, kind (video | audio), name, enabled, locked,
            clips: Clip[]
                  id, name, start, duration, enabled, link?,
                  transform { position: Offset, scale: Scale, rotation,
                              anchor: Offset, opacity }
                  source: ClipSource
                        { type: "media", mediaId, sourceStart }
```

## Time

- `Frame` is an integer count of frames at the frame rate of the sequence the clip is
  in. Each sequence carries its own format, so the same count means different wall time
  in different sequences. Every timeline
  `start`, `duration` and `sourceStart` uses it. There are no float times in the document.
- `Rational { num, den }` holds exact rates such as `24000/1001`.
- Media duration is stored as microseconds (`durationUs`) because that is what ffprobe
  reports. It converts to sequence frames with a floor, so a clip built from it never reads
  past the end of the file.
- Floats appear only when leaving the document: FFmpeg seek arguments and OTIO rates.

## Transform

Every clip carries a `Transform`: where its picture sits in the sequence frame and how
opaque it is. It is the document's half of compositing; the other half is track order.

- Values are **thousandths of their unit** (`Milli`, an `i32`), so `MILLI` (1000) is one
  pixel, one degree or a factor of one. The document holds no floats here for the same
  reason it holds none for time: undo and dirty tracking compare whole documents, and a
  float dragged in from a slider makes that comparison unreliable. One divisor reads
  every field, and `units()` applies it at the tool boundary.
- `position` and `anchor` are `Offset`s: pixels right and down from the **centre** of the
  frame, the way Resolve measures pan and tilt rather than Premiere's absolute Position.
  Zero is therefore the identity, and a centred clip stays centred when the sequence is
  resized, so no default has to be recomputed when the format changes.
- `scale` is a factor of the **fitted** picture, not of the source. Every backend first
  fits the clip into the frame (scaled to fit without cropping, centred), exactly as it
  did before transforms existed, then applies the transform. So the same numbers mean the
  same picture whatever the source resolution is, and `Transform::IDENTITY` renders
  byte-identically to the old behaviour.
- Order is scale, then rotation, both about `anchor`, then translation by `position`.
  Rotation is clockwise.
- `scale` is always positive: a flip is a separate effect, not a negative scale, because
  FFmpeg's `scale` filter needs positive dimensions.

## Invariants

| Rule | Why |
|---|---|
| `schemaVersion` equals the build's `SCHEMA_VERSION` | Older files are migrated on load, never edited in an old shape |
| A project has at least one sequence | There is always something to open, and `first_sequence` cannot fail |
| Every id is unique across project, sequence, media, tracks and clips | Commands address entities by id alone |
| Width and height are even, positive and at most 8192; sample rate is 44.1, 48 or 96 kHz; frame rate is positive and at most 240 | H.264 and ProRes 4:2:2 need even dimensions; these are the rates FFmpeg and NLEs share |
| Clips on a track are sorted by `start` and never overlap | Stacking is between tracks, never within one |
| `start >= 0`, `duration >= 1`, `sourceStart >= 0` | No negative time |
| A clip's media exists, and has a video stream on a video track or an audio stream on an audio track | Render and export can trust every reference |
| `sourceStart + duration` fits inside the media, except for still images | No reading past the end of a file |
| A link group has at least two clips on distinct tracks of one sequence, all with equal `start`, `duration` and `source` | Linked video and audio stay in sync through every edit. Clips in different sequences cannot move together, and their frames are counted at different rates |
| Names are at most 256 characters | Bounded input from the UI and from project files |
| A clip's `scale` is positive and at most 100x, `opacity` is 0 to 1x, `rotation` is within one turn, and `position` and `anchor` stay within four frame widths of centre | Values every backend can actually draw, and two documents showing the same picture hold the same numbers |
| A clip on an audio track has the identity transform | A transform moves a picture; nothing would ever apply one there |

Because every member of a link group shares its timing, an edit either reaches the whole
group or the group has to give up its link. Move, trim, split and delete always apply to
the group. An overwrite applies to it too, so dropping a clip over part of a linked pair
trims or splits both sides and the link survives (both halves of a split pair get a link
of their own). When part of the group sits on a track the overwrite does not clear, a
locked track for example, those clips keep their timing and the link dissolves instead.

## Track order

Tracks live in one list. Order within a kind is stacking order: a later video track
covers an earlier one. The UI shows video tracks top-down from highest to V1, then audio
tracks from A1 downward.

## Sequences

A project holds several. Each carries its own format, so a 25 fps cut and a 24 fps
version can live side by side. Nothing in the document records which sequence a window
is showing: a command names the sequence it acts on, and which one is open is a property
of the window, not of the work (ADR-0004).

Ids are unique across the whole project, so a track or clip id identifies one clip
whichever sequence holds it.

## Extending

- New clip kinds (generated, 3D, motion) are new `ClipSource` variants. Each must bake to a
  media file before export, so OTIO and XML stay media-only.
- Any change to a persisted type bumps `SCHEMA_VERSION` and adds a migration in
  `persist.rs` (see AGENTS.md invariant 4). A migration writes the new shape out rather
  than relying on a serde default, so a document in the editor is always current and a
  file claiming the current version but missing a field is a real error.
