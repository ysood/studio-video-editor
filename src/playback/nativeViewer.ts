// Keeps the native preview surface in step with the UI (ADR-0003).
//
// The surface is an AVPlayerLayer in a native view sitting over the viewer's matte, so
// the web side owns where it goes and when it plays, and AVFoundation owns the picture.

import { useCallback, useEffect, useRef, type RefObject } from "react";

import { ipc } from "../ipc";
import { isBrowserPreview } from "../preview/mode";
import { useEditor } from "../store";

export function useNativeViewer(stage: RefObject<HTMLElement | null>) {
  const snapshot = useEditor((s) => s.snapshot);
  const playing = useEditor((s) => s.playing);
  const playhead = useEditor((s) => s.playhead);
  const lastRect = useRef("");

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

  // The composition is derived, so it is rebuilt after every edit.
  useEffect(() => {
    if (isBrowserPreview) return;
    void ipc.viewerSync();
  }, [snapshot]);

  useEffect(() => {
    if (isBrowserPreview) return;
    if (!playing) {
      void ipc.viewerPause();
      return;
    }
    // Start from where the playhead is, then hand timing to AVFoundation.
    void ipc.viewerSeek(useEditor.getState().playhead).then(() => ipc.viewerPlay());
  }, [playing]);

  // Scrubbing only: during playback AVFoundation runs its own clock and a seek per frame
  // would flush the decoder.
  useEffect(() => {
    if (isBrowserPreview || playing) return;
    void ipc.viewerSeek(playhead);
  }, [playhead, playing]);
}
