import { convertFileSrc } from "@tauri-apps/api/core";
import { useCallback, useEffect, useReducer, useRef } from "react";
import type { MediaAsset } from "../bindings/MediaAsset";
import { framesToSeconds } from "../format";
import { useShortcut } from "../keyboard";
import { useNativeViewer } from "../playback/nativeViewer";
import { audioAt, sequenceEnd, videoStackAt } from "../playback/resolve";
import { isBrowserPreview } from "../preview/mode";
import { useEditor } from "../store";
import { layerStyle } from "../transform";
import { useFraming } from "../viewer/useFraming";
import { HANDLE_SIZE, type OverlayBox } from "../viewer/framing";
import { IconButton, Panel, Timecode } from "../ui";
import "./viewer.css";
import { useActiveSequence } from "../store";

/** Seek a playing element only when it drifts this far, so playback stays smooth. */
const DRIFT_SECONDS = 0.12;

/**
 * The corner marks, each tucked inside the box rather than centred on its corner.
 *
 * A clip that fills the frame has its corners exactly on the edge of the stage, and the
 * stage clips to the matte, so a centred mark would lose its outer half on the most
 * ordinary clip there is. The native surface insets them the same way.
 */
function corners(box: OverlayBox): [number, number][] {
  const inset = HANDLE_SIZE;
  const [right, bottom] = [box.x + box.width - inset, box.y + box.height - inset];
  return [
    [box.x, box.y],
    [right, box.y],
    [right, bottom],
    [box.x, bottom],
  ];
}

/**
 * The clip's box and its corner marks, for the picture path that draws in the web layer.
 *
 * One box and one transform, the same pair the native surface is given, so what is drawn
 * here and what is drawn there come from the same arithmetic.
 */
function Handles({ box }: { box: OverlayBox }) {
  const [a, b, c, d, tx, ty] = box.affine;
  const middle = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  return (
    <svg className="viewer-handles" aria-hidden>
      <g
        // Transform about the middle of the box, which is where a layer turns too.
        transform={`translate(${tx} ${ty}) translate(${middle.x} ${middle.y}) matrix(${a} ${b} ${c} ${d} 0 0) translate(${-middle.x} ${-middle.y})`}
      >
        <rect className="viewer-handle-box" x={box.x} y={box.y} width={box.width} height={box.height} />
        {corners(box).map(([x, y]) => (
          <rect key={`${x},${y}`} className="viewer-handle" x={x} y={y} />
        ))}
      </g>
    </svg>
  );
}

function mediaUrl(media: MediaAsset): string {
  return convertFileSrc(media.path);
}

/** One element per track and file, so a cut within a track reuses what is loaded. */
function layerKey(layer: { track: { id: string }; media: MediaAsset }): string {
  return `${layer.track.id}\u0000${layer.media.id}`;
}

function mediaOf(key: string): string {
  return key.slice(key.indexOf("\u0000") + 1);
}

/**
 * Preview uses the webview's own decoders: one muted <video> or <img> per visual layer
 * and one <audio> per audio clip under the playhead. Every video clip under the
 * playhead is drawn, lowest track at the back, each carrying its own transform as CSS,
 * so a scaled or faded clip shows the layers beneath it the way the renderer does.
 */
export function ViewerPanel() {
  const snapshot = useEditor((s) => s.snapshot);
  const playhead = useEditor((s) => s.playhead);
  const playing = useEditor((s) => s.playing);
  const { setPlayhead, setPlaying } = useEditor.getState();

  const stageRef = useRef<HTMLDivElement>(null);
  // Seeks issued before a video has loaded are dropped, so re-sync once data arrives.
  const [, resync] = useReducer((n: number) => n + 1, 0);
  // Keyed by track and media rather than media alone: two tracks can show the same
  // file at once, and within one track a cut between clips of the same file reuses the
  // element that already has it loaded.
  const visuals = useRef(new Map<string, HTMLVideoElement | HTMLImageElement>());
  const sounds = useRef(new Map<string, HTMLAudioElement>());

  const sequence = useActiveSequence();
  const project = snapshot?.project ?? null;
  const format = sequence?.format;
  const rate = format?.frameRate ?? { num: 24, den: 1 };
  const end = sequence ? sequenceEnd(sequence) : 0;
  const offline = new Set(snapshot?.offlineMedia ?? []);
  const stack = project && sequence ? videoStackAt(project, sequence, playhead) : [];
  const missing = stack.find((a) => offline.has(a.media.id)) ?? null;

  // macOS draws the picture natively; the element path below is the browser fallback.
  useNativeViewer(stageRef);
  // Dragging the clip itself. Works wherever the picture comes from; only the handles
  // are drawn differently (ADR-0006).
  const framing = useFraming(stageRef);

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
    if (!isBrowserPreview || !stage || !project || !format) return;
    const drawn = new Map<string, (typeof stack)[number]>();
    for (const layer of stack) {
      if (!offline.has(layer.media.id)) drawn.set(layerKey(layer), layer);
    }
    for (const [key, layer] of drawn) {
      if (visuals.current.has(key)) continue;
      const element =
        layer.media.kind === "image" ? document.createElement("img") : document.createElement("video");
      element.src = mediaUrl(layer.media);
      element.className = "viewer-media";
      element.addEventListener(element instanceof HTMLVideoElement ? "loadeddata" : "load", resync);
      if (element instanceof HTMLVideoElement) {
        element.muted = true;
        element.preload = "auto";
        element.playsInline = true;
      }
      stage.appendChild(element);
      visuals.current.set(key, element);
    }
    for (const [key, element] of visuals.current) {
      const layer = drawn.get(key);
      element.style.visibility = layer ? "visible" : "hidden";
      if (layer) {
        // Painting order comes from the stack, not from the order elements happened to
        // be created in, so a clip that starts later still sits on its own track.
        const style = layerStyle(layer.clip.transform, format.width, format.height);
        element.style.zIndex = String(stack.indexOf(layer));
        element.style.transformOrigin = style.transformOrigin as string;
        element.style.transform = style.transform as string;
        element.style.opacity = String(style.opacity);
      }
      if (!(element instanceof HTMLVideoElement)) continue;
      if (!layer) {
        if (!element.paused) element.pause();
        continue;
      }
      const target = layer.sourceSeconds;
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
    for (const [key, element] of visuals.current) {
      if (!live.has(mediaOf(key))) {
        element.remove();
        visuals.current.delete(key);
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

  if (!project || !format) return <Panel surface="stage" />;

  return (
    <Panel className="viewer" surface="stage">
      <div className="viewer-surround">
        <div
          ref={stageRef}
          className="viewer-stage"
          data-surface="stage"
          style={{ "--aspect": format.width / format.height } as React.CSSProperties}
          onPointerDown={framing.onPointerDown}
          // A drag is not a click, so releasing after moving a clip must not also start
          // playback.
          onClick={() => {
            if (!framing.dragging) togglePlay();
          }}
        >
          {missing && (
            <div className="viewer-slate">
              <strong>Media offline</strong>
              <span>{missing.media.path}</span>
            </div>
          )}
          {isBrowserPreview && framing.box && <Handles box={framing.box} />}
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
