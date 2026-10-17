import { convertFileSrc } from "@tauri-apps/api/core";
import { useCallback, useEffect, useReducer, useRef } from "react";
import type { MediaAsset } from "../bindings/MediaAsset";
import { framesToSeconds } from "../format";
import { useShortcut } from "../keyboard";
import { useNativeViewer } from "../playback/nativeViewer";
import { audioAt, sequenceEnd, videoAt } from "../playback/resolve";
import { isBrowserPreview } from "../preview/mode";
import { useEditor } from "../store";
import { IconButton, Panel, Timecode } from "../ui";
import "./viewer.css";
import { useActiveSequence } from "../store";

/** Seek a playing element only when it drifts this far, so playback stays smooth. */
const DRIFT_SECONDS = 0.12;

function mediaUrl(media: MediaAsset): string {
  return convertFileSrc(media.path);
}

/**
 * Preview uses the webview's own decoders: one muted <video> or <img> per visual media
 * and one <audio> per audio clip under the playhead. This shows cuts and stacking
 * faithfully; effects will need the GPU compositor described in ADR-0001.
 */
export function ViewerPanel() {
  const snapshot = useEditor((s) => s.snapshot);
  const playhead = useEditor((s) => s.playhead);
  const playing = useEditor((s) => s.playing);
  const { setPlayhead, setPlaying } = useEditor.getState();

  const stageRef = useRef<HTMLDivElement>(null);
  // Seeks issued before a video has loaded are dropped, so re-sync once data arrives.
  const [, resync] = useReducer((n: number) => n + 1, 0);
  const visuals = useRef(new Map<string, HTMLVideoElement | HTMLImageElement>());
  const sounds = useRef(new Map<string, HTMLAudioElement>());

  const sequence = useActiveSequence();
  const project = snapshot?.project ?? null;
  const format = sequence?.format;
  const rate = format?.frameRate ?? { num: 24, den: 1 };
  const end = sequence ? sequenceEnd(sequence) : 0;
  const offline = new Set(snapshot?.offlineMedia ?? []);
  const current = project && sequence ? videoAt(project, sequence, playhead) : null;

  // macOS draws the picture natively; the element path below is the browser fallback.
  useNativeViewer(stageRef);

  // Clock: advance the playhead from wall time while playing.
  //
  // Browser preview only. In the app AVFoundation reports its own time and the playhead
  // follows that (ADR-0003), so there is one clock and the timecode always matches the
  // frame on screen. Wall time here would drift from the picture.
  useEffect(() => {
    if (!isBrowserPreview || !playing || !project) return;
    const startFrame = useEditor.getState().playhead >= end ? 0 : useEditor.getState().playhead;
    const startTime = performance.now();
    const fps = rate.num / rate.den;
    let raf = 0;
    const tick = (now: number) => {
      const frame = startFrame + Math.floor(((now - startTime) / 1000) * fps);
      if (frame >= end) {
        setPlayhead(end);
        setPlaying(false);
        return;
      }
      setPlayhead(frame);
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [playing, project, end, rate.num, rate.den, setPlayhead, setPlaying]);

  // Picture: show the active media element and keep it at the right source time.
  // Browser preview only. In the app AVFoundation owns the picture (ADR-0003), because
  // the asset protocol cannot stream high bitrate media on macOS and an element cannot
  // composite tracks.
  useEffect(() => {
    const stage = stageRef.current;
    if (!isBrowserPreview || !stage || !project) return;
    const activeId = current && !offline.has(current.media.id) ? current.media.id : null;
    if (current && activeId && !visuals.current.has(activeId)) {
      const element =
        current.media.kind === "image" ? document.createElement("img") : document.createElement("video");
      element.src = mediaUrl(current.media);
      element.className = "viewer-media";
      element.addEventListener(element instanceof HTMLVideoElement ? "loadeddata" : "load", resync);
      if (element instanceof HTMLVideoElement) {
        element.muted = true;
        element.preload = "auto";
        element.playsInline = true;
      }
      stage.appendChild(element);
      visuals.current.set(activeId, element);
    }
    for (const [id, element] of visuals.current) {
      const isActive = id === activeId;
      element.style.visibility = isActive ? "visible" : "hidden";
      if (!(element instanceof HTMLVideoElement)) continue;
      if (!isActive) {
        if (!element.paused) element.pause();
        continue;
      }
      const target = current!.sourceSeconds;
      if (playing) {
        if (Math.abs(element.currentTime - target) > DRIFT_SECONDS) element.currentTime = target;
        if (element.paused) void element.play().catch(() => {});
      } else {
        if (!element.paused) element.pause();
        if (Math.abs(element.currentTime - target) > 0.5 / (rate.num / rate.den)) element.currentTime = target;
      }
    }
  });

  // Sound: one element per audible clip, created on demand, dropped when silent.
  // Browser preview only, for the same reason as the picture.
  useEffect(() => {
    if (!isBrowserPreview || !project) return;
    const audible = playing ? audioAt(project, sequence!, playhead).filter((a) => !offline.has(a.media.id)) : [];
    const keep = new Set(audible.map((a) => a.clip.id));
    for (const [id, element] of sounds.current) {
      if (!keep.has(id)) {
        element.pause();
        element.removeAttribute("src");
        sounds.current.delete(id);
      }
    }
    for (const a of audible) {
      let element = sounds.current.get(a.clip.id);
      if (!element) {
        element = new Audio(mediaUrl(a.media));
        element.preload = "auto";
        sounds.current.set(a.clip.id, element);
      }
      if (Math.abs(element.currentTime - a.sourceSeconds) > DRIFT_SECONDS) element.currentTime = a.sourceSeconds;
      if (element.paused) void element.play().catch(() => {});
    }
  });

  // Drop elements for media that left the project.
  useEffect(() => {
    if (!project) return;
    const live = new Set(project.media.map((m) => m.id));
    for (const [id, element] of visuals.current) {
      if (!live.has(id)) {
        element.remove();
        visuals.current.delete(id);
      }
    }
  }, [project]);

  // Starting from the end rewinds first, and writes the rewind to the store rather than
  // keeping it inside the clock. Everything that reacts to `playing` (the native viewer
  // seeks AVFoundation before it starts) then sees the frame playback really begins at.
  const startPlaying = useCallback(() => {
    if (end === 0) return;
    if (useEditor.getState().playhead >= end) setPlayhead(0);
    setPlaying(true);
  }, [end, setPlayhead, setPlaying]);

  const togglePlay = useCallback(() => {
    if (useEditor.getState().playing) {
      setPlaying(false);
      return;
    }
    startPlaying();
  }, [setPlaying, startPlaying]);

  useShortcut(
    useCallback(
      (e: KeyboardEvent) => {
        if (e.metaKey || e.ctrlKey) return false;
        if (e.key === " ") {
          togglePlay();
          return true;
        }
        if (e.key === "k") {
          setPlaying(false);
          return true;
        }
        if (e.key === "l") {
          startPlaying();
          return true;
        }
        return false;
      },
      [togglePlay, setPlaying, startPlaying],
    ),
  );

  if (!project || !format) return <Panel />;

  return (
    <Panel className="viewer">
      <div className="viewer-surround">
        <div
          ref={stageRef}
          className="viewer-stage"
          data-surface="stage"
          style={{ "--aspect": format.width / format.height } as React.CSSProperties}
          onClick={togglePlay}
        >
          {current && offline.has(current.media.id) && (
            <div className="viewer-slate">
              <strong>Media offline</strong>
              <span>{current.media.path}</span>
            </div>
          )}
        </div>
      </div>
      <div className="viewer-transport">
        <Timecode frame={playhead} rate={rate} label="Playhead" />
        <div className="viewer-buttons">
          <IconButton icon="toStart" label="Go to start (Home)" onClick={() => setPlayhead(0)} />
          <IconButton icon="stepBack" label="Back one frame (←)" onClick={() => setPlayhead(playhead - 1)} />
          <IconButton
            icon={playing ? "pause" : "play"}
            shape="round"
            label={playing ? "Pause (Space)" : "Play (Space)"}
            onClick={togglePlay}
            disabled={end === 0}
          />
          <IconButton icon="stepForward" label="Forward one frame (→)" onClick={() => setPlayhead(playhead + 1)} />
          <IconButton icon="toEnd" label="Go to end (End)" onClick={() => setPlayhead(end)} />
        </div>
        <span className="viewer-duration" title={`${framesToSeconds(end, rate).toFixed(2)} s`}>
          <Timecode frame={end} rate={rate} size="body" tone="muted" label="Duration" />
        </span>
      </div>
    </Panel>
  );
}
