# ADR-0005: The video stack composites, instead of the highest track winning

Status: Accepted
Date: 2026-10-20
Deciders: yuvraj

## Context

Milestone 1 had no transforms, and three places independently decided what the picture
is at a given frame:

- `export::render::plan` padded each clip out to the sequence frame with opaque black
  before overlaying it, so a higher video track covered everything below it.
- `viewer::segments` (the macOS preview) flattened every video track into one
  composition track and dropped whatever a higher track covered.
- `playback/resolve.ts` (the browser preview) returned the single topmost clip.

All three agreed, and while every clip filled the frame they were also right: a clip on
V2 really did hide V1. That made the track stack a winner-takes-all rather than a
compositor.

Giving clips a transform breaks the agreement. A clip scaled to half the frame, turned,
or faded has to let the layers below it through. Without compositing, scaling a clip on
V2 fills the rest of the frame with black, which is not what any editor does and not
what the track order in `docs/schema.md` promises.

## Options

1. Transforms without compositing: keep topmost-wins, let a scaled clip reveal black
   + Much smaller change, and correct whenever a clip is alone on V1
   − Wrong the moment there are two video tracks, which is the point of transforms.
   The track stack would mean one thing in the document and another on screen
2. Composite in the renderer only, leave both previews winner-takes-all
   + The exported file is right, which is what ships
   − The preview would stop matching the export exactly where the user is working.
   Studio's whole preview argument (ADR-0003) is that what plays is what exports
3. Composite everywhere: renderer, macOS preview, browser preview
   + One model in all three places, and the stack means what the schema says
   − Three implementations to keep in step, one of them in unsafe AppKit code
4. Composite everywhere, via the wgpu renderer ADR-0001 holds in reserve
   + One implementation instead of three, full control over effects later
   − Months of work before anything plays, for a feature a plain affine covers

## Decision

Option 3. Every clip is composited at its own size, and whatever it does not cover shows
the layer underneath.

The basic transform set (position, scale, rotation, anchor, opacity) is a plain affine
plus alpha, which all three backends already express:

- FFmpeg: `scale`, `rotate` with transparent corners, `colorchannelmixer` for alpha, and
  an `overlay` offset. The opaque pad simply goes away.
- AVFoundation: one composition track per video track, one
  `AVVideoCompositionLayerInstruction` each, with `setTransform:atTime:` and
  `setOpacity:atTime:`. No custom `AVVideoCompositing` is needed, which is the line
  ADR-0003 said would be the point to reconsider.
- The webview: one positioned element per layer with a CSS `transform` and `opacity`.

The arithmetic is stated once in `docs/schema.md` and implemented three times against
the same cases: scale and rotation turn about the anchor, position places the anchor
(Premiere's convention), rotation is clockwise.

## Consequences

- Three implementations now mirror each other rather than one rule being repeated, which
  is the cost ADR-0003 already accepted for the preview and this extends. They are
  pinned by tests that assert the same transforms land in the same place, and the
  renderer has an end-to-end test that a scaled, faded clip on V2 shows V1 at the corner.
- An identity transform renders exactly what it rendered before: the fit happens first
  and every transform filter whose value is the identity is left out of the graph.
- The macOS preview holds more composition tracks and draws more layers, so a tall stack
  costs more than it did. It was free before because the covered clips were thrown away.
- Transforms travel to Resolve and Premiere through FCP7 XML's `Basic Motion` and
  `Opacity` effects, written only when a clip has something to say so an untransformed
  cut exports the file it always did. Their units are fractions of the frame measured
  from its middle, which is how the format stays resolution independent when media is
  relinked. Apple's reference does not state the convention; it is taken from a Premiere
  export, so the first real import is worth checking by eye.
- `Basic Motion` carries a single Scale, so a clip scaled differently across and down
  reaches Premiere as its width scale. Nothing in the format holds the other one.
- OTIO gets the transform as `metadata.studio.transform`, in the document's own
  thousandths, rather than an effect. OTIO has no schema for an affine transform
  (OTIO issue 253) and its maintainers' advice for anything the format does not model is
  to carry it as metadata, so a reader that does not know Studio ignores it instead of
  mis-drawing the clip. Emit a schema instead if OTIO ever grows one.
- Direct manipulation in the viewer (drag and corner handles) is still out of reach on
  macOS: the picture is a native view above the webview (ADR-0003), so a web overlay
  cannot be drawn on top of it. That needs either a second native view or handles drawn
  natively, and is its own decision.
- A flip is not a negative scale. Scale stays positive because FFmpeg's `scale` filter
  needs positive dimensions; a flip will be its own effect.
