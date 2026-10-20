// Dragging a clip in the viewer (ADR-0006).
//
// The gesture is the same wherever the picture comes from: the web layer decides what a
// pointer is over and what the drag does, and the only difference is who draws the
// handles. On macOS the picture is a native view above the webview, so the overlay is
// pushed to it and mouse events come back as `viewer://pointer`; in the browser preview
// the handles are SVG over the stage and the events are the DOM's own.
//
// One edit per gesture, committed on release, so undo steps through drags rather than
// through pointer moves. While the drag runs only the outline moves: the picture
// follows when the edit lands, the way a timeline drag shows a ghost and the clip moves
// on drop.

import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import type { Clip } from "../bindings/Clip";
import type { Track } from "../bindings/Track";
import { setTransform } from "../edits";
import { events, ipc } from "../ipc";
import { isBrowserPreview } from "../preview/mode";
import { useActiveSequence, useEditor } from "../store";
import { findClip } from "../timeline/geometry";
import {
  changed,
  clipQuad,
  dragged,
  grabAt,
  overlayBox,
  sourceSize,
  type DragStart,
  type OverlayBox,
  type Point,
  type Size,
} from "./framing";

/** The one video clip the handles belong to, or null when they do not belong anywhere. */
function framedClip(
  selection: string[],
  sequence: ReturnType<typeof useActiveSequence>,
): { clip: Clip; track: Track } | null {
  if (!sequence) return null;
  const found = selection
    .map((id) => findClip(sequence, id))
    .filter((f): f is { clip: Clip; track: Track } => f !== null)
    .filter((f) => f.track.kind === "video");
  return found.length === 1 ? found[0]! : null;
}

export interface Framing {
  /** Where to draw the handles, or null for none. Stage pixels, y from the top. */
  box: OverlayBox | null;
  /** True while a drag is running, so the stage can suppress its click. */
  dragging: boolean;
  onPointerDown: (event: React.PointerEvent<HTMLElement>) => void;
}

export function useFraming(stage: RefObject<HTMLElement | null>): Framing {
  const selection = useEditor((s) => s.selection);
  const sequence = useActiveSequence();
  const snapshot = useEditor((s) => s.snapshot);
  const entry = framedClip(selection, sequence);
  const format = sequence?.format ?? null;
  const media = entry
    ? (snapshot?.project.media.find((m) => m.id === entry.clip.source.mediaId) ?? null)
    : null;
  const locked = entry?.track.locked ?? false;

  // The transform the outline shows: the clip's, or where the drag has got to.
  const [preview, setPreview] = useState<DragStart["transform"] | null>(null);
  const drag = useRef<DragStart | null>(null);

  const stageSize = useCallback((): Size | null => {
    const element = stage.current;
    if (!element) return null;
    const rect = element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0 ? { width: rect.width, height: rect.height } : null;
  }, [stage]);

  const transform = preview ?? entry?.clip.transform ?? null;
  const size = stageSize();
  const box =
    transform && media && format && size ? overlayBox(transform, sourceSize(media, format), format, size) : null;

  // Act on a pointer, wherever it came from. Kept in a ref so the native listener does
  // not have to be torn down and rebuilt every time the selection changes.
  const act = useRef<(phase: string, at: Point, shift: boolean) => void>(() => {});
  act.current = (phase, at, shift) => {
    if (!entry || !media || !format) return;
    const stageNow = stageSize();
    if (!stageNow) return;
    const source = sourceSize(media, format);

    if (phase === "down") {
      if (locked) return;
      const quad = clipQuad(entry.clip.transform, source, { width: format.width, height: format.height });
      const grab = grabAt(at, quad, { width: format.width, height: format.height }, stageNow);
      if (!grab) return;
      drag.current = { grab, transform: entry.clip.transform, at };
      setPreview(entry.clip.transform);
      return;
    }

    const started = drag.current;
    if (!started) return;
    const next = dragged(started, at, source, format, stageNow, { perAxis: shift });
    if (phase === "moved") {
      setPreview(next);
      return;
    }
    drag.current = null;
    setPreview(null);
    // Nothing to undo for a click that did not move anything.
    if (changed(started.transform, next)) {
      void useEditor.getState().edit(
        setTransform([entry.clip.id], {
          position: next.position,
          scale: next.scale,
          rotation: next.rotation,
          anchor: next.anchor,
          opacity: next.opacity,
        }),
      );
    }
  };

  // Native: the surface reports the mouse, because it covers the webview.
  useEffect(() => {
    if (isBrowserPreview) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void events
      .onViewerPointer((pointer) => {
        act.current(pointer.phase, { x: pointer.x, y: pointer.y }, pointer.shift);
      })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // Native: push the outline whenever it moves. AppKit measures y from the bottom of
  // the surface, which is the same conversion `viewerSetRect` already makes.
  const pushed = useRef("");
  useEffect(() => {
    if (isBrowserPreview) return;
    const size = stageSize();
    const payload =
      box && size
        ? {
            x: box.x,
            y: size.height - (box.y + box.height),
            width: box.width,
            height: box.height,
            // The affine was worked out with y downwards, so flipping the space flips
            // the two terms that cross the axes, and the vertical shift.
            transform: [
              box.affine[0],
              -box.affine[1],
              -box.affine[2],
              box.affine[3],
              box.affine[4],
              -box.affine[5],
            ] as [number, number, number, number, number, number],
          }
        : null;
    const key = JSON.stringify(payload);
    if (key === pushed.current) return;
    pushed.current = key;
    void ipc.viewerSetOverlay(payload);
  }, [box, stageSize]);

  // Browser preview: the DOM reports the mouse itself, and the pointer is captured so a
  // drag that leaves the stage still finishes.
  const onPointerDown = useCallback(
    (event: React.PointerEvent<HTMLElement>) => {
      const element = stage.current;
      if (!element) return;
      const rect = element.getBoundingClientRect();
      const at = { x: event.clientX - rect.left, y: event.clientY - rect.top };
      act.current("down", at, event.shiftKey);
      if (!drag.current) return;
      event.preventDefault();
      element.setPointerCapture(event.pointerId);

      const move = (e: PointerEvent) => {
        const r = element.getBoundingClientRect();
        act.current("moved", { x: e.clientX - r.left, y: e.clientY - r.top }, e.shiftKey);
      };
      const up = (e: PointerEvent) => {
        const r = element.getBoundingClientRect();
        act.current("up", { x: e.clientX - r.left, y: e.clientY - r.top }, e.shiftKey);
        element.releasePointerCapture(e.pointerId);
        element.removeEventListener("pointermove", move);
        element.removeEventListener("pointerup", up);
      };
      element.addEventListener("pointermove", move);
      element.addEventListener("pointerup", up);
    },
    [stage],
  );

  return { box, dragging: preview !== null, onPointerDown };
}
