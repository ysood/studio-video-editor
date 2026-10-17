// Keeps the native preview surface in step with the UI (ADR-0003).
//
// The surface is an AVPlayerLayer in a native view sitting over the viewer's matte. The
// web side owns where it goes and when it plays; AVFoundation owns the picture and the
// clock. The playhead follows the player rather than wall time, so there is one clock
// and the number on screen always matches the frame being shown.

import { useCallback, useEffect, useRef, type RefObject } from "react";

import { microsToFrames } from "../format";
import { events, ipc } from "../ipc";
import { isBrowserPreview } from "../preview/mode";
import { useOverlaysOpen } from "../ui/overlay";
import { useActiveSequence, useEditor } from "../store";
import { sequenceEnd } from "./resolve";

export function useNativeViewer(stage: RefObject<HTMLElement | null>) {
  const snapshot = useEditor((s) => s.snapshot);
  const playing = useEditor((s) => s.playing);
  const playhead = useEditor((s) => s.playhead);
  const lastRect = useRef("");
  // Whether the surface is currently out of the way. Held in a ref so the effect that
  // puts it back knows there is something to put back.
  const hidden = useRef(false);
  // The last frame the player reported, so its own updates do not bounce back as seeks.
  const fromPlayer = useRef<number | null>(null);

  const sequence = useActiveSequence();
  const sequenceId = sequence?.id ?? null;
  const rate = sequence?.format.frameRate ?? { num: 24, den: 1 };
  const num = rate.num;
  const den = rate.den;
  const end = sequence ? sequenceEnd(sequence) : 0;

  // The surface is positioned in CSS pixels, which match AppKit points because the
  // webview fills the window. Pushed only when it actually moves, since this runs on
  // every render.
  const pushRect = useCallback(() => {
    const element = stage.current;
    if (isBrowserPreview || !element) return;
    const rect = element.getBoundingClientRect();
    const key = `${rect.x},${rect.y},${rect.width},${rect.height}`;
    if (key === lastRect.current) return;
    lastRect.current = key;
    void ipc.viewerSetRect(rect.x, rect.y, rect.width, rect.height);
  }, [stage]);

  useEffect(pushRect);

  const setHidden = useCallback((next: boolean) => {
    if (hidden.current === next) return;
    hidden.current = next;
    void ipc.viewerSetHidden(next);
  }, []);

  // The surface is a native view above the webview, so it paints over anything the web
  // layer draws in the same place: a dialog over the viewer comes out half buried. While
  // something is open, step aside. A modal always counts, because it takes the whole
  // window; a menu or popover only counts where it actually reaches the picture, so
  // right-clicking a clip does not blank the viewer. Menus move while open, so the test
  // runs per frame, which costs nothing because it only runs while one is open.
  const overlaysOpen = useOverlaysOpen();
  useEffect(() => {
    if (isBrowserPreview) return;
    if (!overlaysOpen) {
      setHidden(false);
      return;
    }
    let frame = 0;
    const tick = () => {
      const element = stage.current;
      if (element) {
        const picture = element.getBoundingClientRect();
        setHidden(
          document.querySelector("dialog[open]") !== null ||
            [...document.querySelectorAll(".ui-overlay")].some((node) => {
              const over = node.getBoundingClientRect();
              return (
                over.width > 0 &&
                over.height > 0 &&
                over.left < picture.right &&
                over.right > picture.left &&
                over.top < picture.bottom &&
                over.bottom > picture.top
              );
            }),
        );
      }
      frame = requestAnimationFrame(tick);
    };
    tick();
    return () => cancelAnimationFrame(frame);
  }, [overlaysOpen, stage, setHidden]);

  // Dockview can resize the panel without re-rendering this component.
  useEffect(() => {
    const element = stage.current;
    if (isBrowserPreview || !element) return;
    const observer = new ResizeObserver(pushRect);
    observer.observe(element);
    window.addEventListener("resize", pushRect);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", pushRect);
    };
  }, [stage, pushRect]);

  // The composition is derived, so it is rebuilt after every edit, and again when
  // another sequence is opened. The playhead goes with it: rebuilding replaces the
  // player's item, which would otherwise leave the viewer showing the first frame of the
  // timeline instead of the frame you are on.
  useEffect(() => {
    if (isBrowserPreview || !sequenceId) return;
    void ipc.viewerSync(sequenceId, useEditor.getState().playhead);
  }, [snapshot, sequenceId]);

  // Follow the player's clock while it runs, and stop when it reaches the end.
  useEffect(() => {
    if (isBrowserPreview) return;
    const { setPlayhead, setPlaying } = useEditor.getState();
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void events
      .onViewerTime((micros: number) => {
        if (!useEditor.getState().playing) return;
        const frame = Math.min(microsToFrames(micros, { num, den }), end);
        fromPlayer.current = frame;
        setPlayhead(frame);
        if (frame >= end) setPlaying(false);
      })
      .then((fn: () => void) => {
        if (cancelled) fn();
        else unlisten = fn;
      });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [num, den, end]);

  useEffect(() => {
    if (isBrowserPreview) return;
    if (!playing || !sequenceId) {
      void ipc.viewerPause();
      return;
    }
    // Start from where the playhead is, then hand timing to AVFoundation.
    void ipc.viewerSeek(sequenceId, useEditor.getState().playhead).then(() => ipc.viewerPlay());
  }, [playing, sequenceId]);

  // Scrubbing only. During playback the player is the clock, and a seek per reported
  // frame would flush the decoder.
  useEffect(() => {
    if (isBrowserPreview || playing || !sequenceId) return;
    if (fromPlayer.current === playhead) return;
    void ipc.viewerSeek(sequenceId, playhead);
  }, [playhead, playing, sequenceId]);
}
