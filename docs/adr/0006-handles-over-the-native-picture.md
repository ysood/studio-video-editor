# ADR-0006: Handles are drawn by the native surface, decided by the web layer

Status: Accepted
Date: 2026-10-20
Deciders: yuvraj

## Context

Clips carry a transform (ADR-0005) but it can only be typed into the Transform panel.
Framing a shot by typing numbers is not how anyone frames a shot, so the clip has to be
draggable in the viewer.

The obstacle is where the picture comes from. On macOS it is an `AVPlayerLayer` in an
`NSView` added **above** the `WKWebView` and positioned from the web UI (ADR-0003). That
ordering was chosen because making the webview transparent is unreliable on recent macOS
(wry#1867), and because nothing web-drawn needed to sit over the picture. Handles do.

Nothing the web layer draws over the picture is visible, so handles have to come from
somewhere else.

Input turned out to be a different matter. This ADR first claimed the native view wins
the hit test and a click on the stage never reaches the webview. Driving the built app
showed otherwise: the webview still receives the press underneath the surface, which is
why `onClick={togglePlay}` on the stage has been working on macOS all along. The
correction matters, because it is what made the first version of the gesture wrong.

## Options

1. Draw handles in the web layer and change nothing native
   + No Objective-C at all; works today in the browser preview
   − Invisible and unclickable on macOS, which is the platform the preview is built for
2. Hide the native surface for the duration of a drag, draw everything in the web layer
   + One call that already exists (`set_hidden`, used for dialogs over the viewer); no
   native drawing code; one set of handles for both picture paths
   − You frame the shot without seeing it. The picture goes black exactly while you are
   deciding where to put it
2a. Draw natively but let the webview keep the input, now that it is known to get it
   + No event monitor at all
   − Untested: a press is known to arrive, a drag over the native picture is not. Worth
   revisiting, because it would delete the monitor outright
3. Put the player layer below the webview and make the webview transparent
   + The web layer owns everything, as it would on any other platform
   − ADR-0003 rejected this on evidence (wry#1867), and nothing has changed
4. The native surface draws a thin overlay and reports pointer events; the web layer
   decides everything
   + The picture stays visible under the handles; all the logic stays in TypeScript next
   to the other drag previews
   − More Objective-C bridging, and a fourth place that has to agree about the transform

## Decision

Option 4, with the split drawn tightly: **Rust draws dumb shapes and reports dumb
points. It decides nothing.**

- The surface view's layer becomes a plain `CALayer` host. The `AVPlayerLayer` is a
  sublayer sized to the view, and an overlay `CALayer` sits above it holding a bordered
  rectangle and four corner marks. The web layer sends one quad; Rust positions it.
- Pointer events come from an `NSEvent` local monitor rather than an `NSView` subclass,
  so no Objective-C class has to be declared and the responder chain is left alone. The
  monitor converts the location into the surface's own coordinates, flips y to match CSS,
  and emits `viewer://pointer`. The event is returned unchanged, so nothing else breaks.
- Hit testing, the drag arithmetic, the preview and the commit are all in
  `src/viewer/framing.ts`, which is pure and unit tested, exactly as
  `src/timeline/geometry.ts` is for timeline drags.

**During a drag the picture does not move.** An outline shows where the clip is going and
the picture updates when the pointer is released. That mirrors the timeline, where a drag
draws a ghost and the clip moves on drop, and it avoids rebuilding the video composition
on every pointer move, which is the only way AVFoundation would show the motion live.

A corner drag scales uniformly; Shift scales each axis on its own. Uniform is the default
because it is what the Transform panel defaults to and what a modern canvas tool does.

## Consequences

- Two sources of input now exist over the stage: the monitor, and the webview, which gets
  the press anyway. Exactly one may drive a gesture. The first version let the webview
  start the drag and the monitor finish it, and because their coordinates do not agree,
  every drag gained a constant 74 frame pixels of y. The DOM handler is therefore bound
  to the browser preview and the monitor owns the gesture in the app.
- The monitor is kept rather than dropped in favour of the webview's own events, because
  a press is known to arrive there but a drag across the native picture has not been
  tested. Option 2a above is the thing to measure if this is revisited.
- A fourth implementation now has to agree about the transform: the renderer, the macOS
  preview, the browser preview and now the handles. The arithmetic is in one tested
  module and `placePoint` is checked against the same cases as the other three.
- The handle box is the picture's fitted rectangle, which needs the media's stored
  dimensions. Footage a camera recorded sideways reports the size it is stored at, so the
  box can be wrong for that clip even though the picture is drawn correctly: the
  renderer and the preview both get the real size from the decoder, and this does not.
  Visible only as a misplaced handle, so it is a known limit rather than a blocker.
- Rotation and the anchor stay in the Transform panel. Rotation handles would need a
  second gesture on the same corners, and the anchor needs a fifth mark; neither earns
  its place in the first version.
- The browser preview path uses the same module driven by DOM pointer events, so there is
  one set of rules and one set of tests, and the only difference is who draws.
