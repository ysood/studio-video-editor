import { convertFileSrc } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef } from "react";
import type { MediaAsset } from "../bindings/MediaAsset";
import { framesToSeconds, timecode } from "../format";
import { useShortcut } from "../keyboard";
import { audioAt, sequenceEnd, videoAt } from "../playback/resolve";
import { useEditor } from "../store";
import "./viewer.css";

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
  const visuals = useRef(new Map<string, HTMLVideoElement | HTMLImageElement>());
  const sounds = useRef(new Map<string, HTMLAudioElement>());

  const project = snapshot?.project ?? null;
  const format = project?.sequence.format;
  const rate = format?.frameRate ?? { num: 24, den: 1 };
  const end = project ? sequenceEnd(project) : 0;
  const offline = new Set(snapshot?.offlineMedia ?? []);
  const current = project ? videoAt(project, playhead) : null;

  // Clock: advance the playhead from wall time while playing.
  useEffect(() => {
    if (!playing || !project) return;
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
  useEffect(() => {
    const stage = stageRef.current;
    if (!stage || !project) return;
    const activeId = current && !offline.has(current.media.id) ? current.media.id : null;
    if (current && activeId && !visuals.current.has(activeId)) {
      const element =
        current.media.kind === "image" ? document.createElement("img") : document.createElement("video");
      element.src = mediaUrl(current.media);
      element.className = "viewer-media";
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
  useEffect(() => {
    if (!project) return;
    const audible = playing ? audioAt(project, playhead).filter((a) => !offline.has(a.media.id)) : [];
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

  const togglePlay = useCallback(() => {
    if (end > 0) setPlaying(!useEditor.getState().playing);
  }, [end, setPlaying]);

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
          if (end > 0) setPlaying(true);
          return true;
        }
        return false;
      },
      [togglePlay, setPlaying, end],
    ),
  );

  if (!project || !format) return <div className="panel" />;

  return (
    <div className="panel viewer">
      <div className="viewer-surround">
        <div
          ref={stageRef}
          className="viewer-stage"
          style={{ aspectRatio: `${format.width} / ${format.height}` }}
          onClick={togglePlay}
        >
          {current && offline.has(current.media.id) && (
            <div className="viewer-slate">
              <strong>Media offline</strong>
              <span>{current.media.path}</span>
            </div>
          )}
          {end === 0 && (
            <div className="viewer-slate">
              <span>Picture appears here once the timeline has clips.</span>
            </div>
          )}
        </div>
      </div>
      <div className="viewer-transport">
        <span className="viewer-timecode tabular">{timecode(playhead, rate)}</span>
        <div className="viewer-buttons">
          <button className="icon-btn" title="Go to start (Home)" onClick={() => setPlayhead(0)} aria-label="Go to start">
            ⏮
          </button>
          <button className="icon-btn" title="Back one frame (←)" onClick={() => setPlayhead(playhead - 1)} aria-label="Back one frame">
            ◀︎
          </button>
          <button
            className="viewer-play"
            onClick={togglePlay}
            disabled={end === 0}
            aria-label={playing ? "Pause" : "Play"}
            title="Play or pause (Space)"
          >
            {playing ? "❚❚" : "▶︎"}
          </button>
          <button className="icon-btn" title="Forward one frame (→)" onClick={() => setPlayhead(playhead + 1)} aria-label="Forward one frame">
            ▶︎
          </button>
          <button className="icon-btn" title="Go to end (End)" onClick={() => setPlayhead(end)} aria-label="Go to end">
            ⏭
          </button>
        </div>
        <span className="viewer-duration tabular" title={`${framesToSeconds(end, rate).toFixed(2)} s`}>
          {timecode(end, rate)}
        </span>
      </div>
    </div>
  );
}
