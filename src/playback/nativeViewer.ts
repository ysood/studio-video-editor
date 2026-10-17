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
import { useEditor } from "../store";
import { sequenceEnd } from "./resolve";
import { firstSequence } from "../sequences";

export function useNativeViewer(stage: RefObject<HTMLElement | null>) {
  const snapshot = useEditor((s) => s.snapshot);
  const playing = useEditor((s) => s.playing);
  const playhead = useEditor((s) => s.playhead);
  const lastRect = useRef("");
  // The last frame the player reported, so its own updates do not bounce back as seeks.
  const fromPlayer = useRef<number | null>(null);

  const project = snapshot?.project ?? null;
  const rate = project ? firstSequence(project).format.frameRate : { num: 24, den: 1 };
  const num = rate.num;
  const den = rate.den;
  const end = project ? sequenceEnd(project) : 0;

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

  // The composition is derived, so it is rebuilt after every edit. The playhead goes
  // with it: rebuilding replaces the player's item, which would otherwise leave the
  // viewer showing the first frame of the timeline instead of the frame you are on.
  useEffect(() => {
    if (isBrowserPreview) return;
    void ipc.viewerSync(useEditor.getState().playhead);
  }, [snapshot]);

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
    if (!playing) {
      void ipc.viewerPause();
      return;
    }
    // Start from where the playhead is, then hand timing to AVFoundation.
    void ipc.viewerSeek(useEditor.getState().playhead).then(() => ipc.viewerPlay());
  }, [playing]);

  // Scrubbing only. During playback the player is the clock, and a seek per reported
  // frame would flush the decoder.
  useEffect(() => {
    if (isBrowserPreview || playing) return;
    if (fromPlayer.current === playhead) return;
    void ipc.viewerSeek(playhead);
  }, [playhead, playing]);
}
