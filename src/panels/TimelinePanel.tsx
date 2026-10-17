import type { IDockviewPanelProps } from "dockview-react";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { ClipEdge } from "../bindings/ClipEdge";
import type { ClipId } from "../bindings/ClipId";
import type { Project } from "../bindings/Project";
import type { Track } from "../bindings/Track";
import type { TrackId } from "../bindings/TrackId";
import { firstTrack, insertMedia, MEDIA_DRAG_TYPE, mediaDrag } from "../edits";
import { microsToFrames, timecode } from "../format";
import { useShortcut } from "../keyboard";
import { useEditor, ZOOM_MAX, ZOOM_MIN } from "../store";
import { draw, type Ghost } from "../timeline/draw";
import {
  clipEnd,
  contentHeight,
  findClip,
  frameToX,
  hitTest,
  layoutRows,
  linkGroup,
  previewMove,
  previewTrim,
  rowAt,
  SNAP_PX,
  xToFrame,
  type View,
} from "../timeline/geometry";
import "./timeline.css";

/** Pointer travel before a press on a clip becomes a drag, so clicks stay clicks. */
const DRAG_THRESHOLD_PX = 3;

type Drag =
  | { kind: "scrub" }
  | { kind: "move"; clipId: ClipId; grabOffset: number; startX: number; active: boolean; start: number; trackId: TrackId }
  | { kind: "trim"; clipId: ClipId; edge: ClipEdge; frame: number };

interface DropPreview {
  ghosts: Ghost[];
  start: number;
  video: TrackId | null;
  audio: TrackId | null;
}

function mediaLength(project: Project, mediaId: string): number {
  const media = project.media.find((m) => m.id === mediaId);
  const rate = project.sequence.format.frameRate;
  if (!media) return 0;
  if (media.durationUs === null) return Math.floor((5 * rate.num) / rate.den);
  return microsToFrames(media.durationUs, rate);
}

export function TimelinePanel({ api }: IDockviewPanelProps) {
  const snapshot = useEditor((s) => s.snapshot);
  const selection = useEditor((s) => s.selection);
  const playhead = useEditor((s) => s.playhead);
  const zoom = useEditor((s) => s.zoom);
  const scrollX = useEditor((s) => s.scrollX);
  const { edit, select, setPlayhead, setZoom, setScrollX } = useEditor.getState();

  const canvasRef = useRef<HTMLCanvasElement>(null);
  const areaRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(800);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [drop, setDrop] = useState<DropPreview | null>(null);
  const [snap, setSnap] = useState(true);

  const project = snapshot?.project ?? null;
  const rows = useMemo(() => (project ? layoutRows(project) : []), [project]);
  const height = contentHeight(rows);
  const view: View = useMemo(() => ({ zoom, scrollX }), [zoom, scrollX]);
  const rate = project?.sequence.format.frameRate ?? { num: 24, den: 1 };
  const selectionSet = useMemo(() => new Set(selection), [selection]);

  // Keep the canvas sized to its container. The area only exists once a project has
  // loaded, so the observer attaches then.
  const hasProject = project !== null;
  useLayoutEffect(() => {
    const area = areaRef.current;
    if (!area) return;
    // ResizeObserver alone missed Dockview's initial layout in WebKitGTK, so panel
    // dimension events also trigger a measure.
    const measure = () => setWidth(area.clientWidth);
    const observer = new ResizeObserver(measure);
    observer.observe(area);
    const dimensions = api.onDidDimensionsChange(() => requestAnimationFrame(measure));
    window.addEventListener("resize", measure);
    measure();
    return () => {
      observer.disconnect();
      dimensions.dispose();
      window.removeEventListener("resize", measure);
    };
  }, [hasProject, api]);

  // Page the view along with the playhead during playback.
  const playing = useEditor((s) => s.playing);
  useEffect(() => {
    if (!playing) return;
    const x = frameToX(playhead, view);
    if (x > width - 24 || x < 0) setScrollX(playhead * zoom - 24);
  }, [playing, playhead, view, width, zoom, setScrollX]);

  // Ghosts for an in-progress move or trim.
  const { ghosts, lifted } = useMemo(() => {
    const ghosts: Ghost[] = [];
    const lifted = new Set<ClipId>();
    if (!project || !drag || drag.kind === "scrub") return { ghosts, lifted };
    if (drag.kind === "move" && drag.active) {
      const dragged = findClip(project, drag.clipId);
      if (!dragged) return { ghosts, lifted };
      const delta = drag.start - dragged.clip.start;
      for (const id of linkGroup(project, drag.clipId)) {
        const found = findClip(project, id);
        if (!found) continue;
        lifted.add(id);
        ghosts.push({
          trackId: id === drag.clipId ? drag.trackId : found.track.id,
          start: found.clip.start + delta,
          duration: found.clip.duration,
          name: found.clip.name,
          kind: found.track.kind,
        });
      }
    }
    if (drag.kind === "trim") {
      for (const id of linkGroup(project, drag.clipId)) {
        const found = findClip(project, id);
        if (!found) continue;
        const { clip, track } = found;
        lifted.add(id);
        const start = drag.edge === "start" ? drag.frame : clip.start;
        const end = drag.edge === "end" ? drag.frame : clipEnd(clip);
        ghosts.push({ trackId: track.id, start, duration: end - start, name: clip.name, kind: track.kind });
      }
    }
    return { ghosts, lifted };
  }, [project, drag]);

  // Paint.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !project) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.floor(width * dpr);
    canvas.height = Math.floor(height * dpr);
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    draw(ctx, {
      project,
      rows,
      view,
      width,
      height,
      playhead,
      selection: selectionSet,
      offline: new Set(snapshot?.offlineMedia ?? []),
      lifted,
      ghosts: [...ghosts, ...(drop?.ghosts ?? [])],
    });
  }, [project, rows, view, width, height, playhead, selectionSet, ghosts, lifted, drop, snapshot]);

  const pointer = (e: { clientX: number; clientY: number }) => {
    const rect = canvasRef.current!.getBoundingClientRect();
    return { x: e.clientX - rect.left, y: e.clientY - rect.top };
  };

  const snapThreshold = SNAP_PX / zoom;

  const onPointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (!project || e.button !== 0) return;
    const { x, y } = pointer(e);
    const hit = hitTest(rows, view, x, y);
    e.currentTarget.setPointerCapture(e.pointerId);
    if (hit.type === "ruler") {
      setPlayhead(xToFrame(x, view));
      setDrag({ kind: "scrub" });
      return;
    }
    if (hit.type !== "clip") {
      if (!e.shiftKey && !e.metaKey) select([]);
      return;
    }
    // Linked clips select together, as they move together. Alt selects one side only.
    const picked = e.altKey ? [hit.clip.id] : linkGroup(project, hit.clip.id);
    if (e.shiftKey || e.metaKey) select(picked, "toggle");
    else if (!selectionSet.has(hit.clip.id)) select(picked);
    if (hit.track.locked) return;
    if (hit.zone === "body") {
      setDrag({
        kind: "move",
        clipId: hit.clip.id,
        grabOffset: xToFrame(x, view) - hit.clip.start,
        startX: x,
        active: false,
        start: hit.clip.start,
        trackId: hit.track.id,
      });
    } else {
      const frame = hit.zone === "start" ? hit.clip.start : clipEnd(hit.clip);
      setDrag({ kind: "trim", clipId: hit.clip.id, edge: hit.zone, frame });
    }
  };

  const onPointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (!project) return;
    const { x, y } = pointer(e);
    if (!drag) {
      const hit = hitTest(rows, view, x, y);
      e.currentTarget.style.cursor =
        hit.type === "clip" && !hit.track.locked
          ? hit.zone === "body"
            ? "grab"
            : "ew-resize"
          : hit.type === "ruler"
            ? "text"
            : "default";
      return;
    }
    if (drag.kind === "scrub") {
      setPlayhead(xToFrame(x, view));
    } else if (drag.kind === "move") {
      const active = drag.active || Math.abs(x - drag.startX) > DRAG_THRESHOLD_PX;
      if (!active) return;
      const over: Track | null = rowAt(rows, y)?.track ?? null;
      const preview = previewMove(project, drag.clipId, xToFrame(x, view) - drag.grabOffset, over, {
        enabled: snap,
        playhead,
        thresholdFrames: snapThreshold,
      });
      if (preview) setDrag({ ...drag, active: true, start: preview.start, trackId: preview.trackId });
    } else if (drag.kind === "trim") {
      let raw = xToFrame(x, view);
      if (snap && Math.abs(raw - playhead) <= snapThreshold) raw = playhead;
      const frame = previewTrim(project, drag.clipId, drag.edge, raw);
      if (frame !== null) setDrag({ ...drag, frame });
    }
  };

  const onPointerUp = () => {
    if (!project || !drag) return;
    setDrag(null);
    if (drag.kind === "move" && drag.active) {
      const found = findClip(project, drag.clipId);
      if (found && (found.clip.start !== drag.start || found.track.id !== drag.trackId)) {
        void edit({
          type: "MoveClip",
          clipId: drag.clipId,
          start: drag.start,
          trackId: drag.trackId === found.track.id ? null : drag.trackId,
        });
      }
    } else if (drag.kind === "trim") {
      void edit({ type: "TrimClip", clipId: drag.clipId, edge: drag.edge, frame: drag.frame });
    }
  };

  const onWheel = (e: React.WheelEvent) => {
    if (e.ctrlKey || e.metaKey) {
      // Zoom around the pointer so the frame under it stays put.
      const { x } = pointer(e);
      const anchor = xToFrame(x, view);
      const next = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, zoom * Math.exp(-e.deltaY * 0.01)));
      setZoom(next);
      setScrollX(anchor * next - x);
      return;
    }
    const dx = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? e.deltaX : e.shiftKey ? e.deltaY : 0;
    if (dx !== 0) setScrollX(scrollX + dx);
  };

  // Bin drops.
  const dropTargets = (y: number, mediaId: string) => {
    if (!project) return null;
    const media = project.media.find((m) => m.id === mediaId);
    if (!media) return null;
    const over = rowAt(rows, y)?.track;
    const usable = (t: Track | undefined) => (t && !t.locked ? t.id : null);
    const video = media.video ? (over?.kind === "video" ? usable(over) : firstTrack(project, "video")) : null;
    const audio = media.audio ? (over?.kind === "audio" ? usable(over) : firstTrack(project, "audio")) : null;
    return { media, video, audio };
  };

  const onDragOver = (e: React.DragEvent) => {
    if (!project || !e.dataTransfer.types.includes(MEDIA_DRAG_TYPE) || !mediaDrag.current) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
    const { x, y } = pointer(e);
    const targets = dropTargets(y, mediaDrag.current);
    if (!targets) return;
    const duration = mediaLength(project, targets.media.id);
    let start = Math.max(0, Math.round(xToFrame(x, view)));
    if (snap) {
      const near = [0, playhead, ...project.sequence.tracks.flatMap((t) => t.clips.flatMap((c) => [c.start, clipEnd(c)]))];
      const hit = near.find((f) => Math.abs(f - start) <= snapThreshold);
      if (hit !== undefined) start = hit;
    }
    const ghosts: Ghost[] = [];
    if (targets.video) ghosts.push({ trackId: targets.video, start, duration, name: targets.media.name, kind: "video" });
    if (targets.audio) ghosts.push({ trackId: targets.audio, start, duration, name: targets.media.name, kind: "audio" });
    setDrop({ ghosts, start, video: targets.video, audio: targets.audio });
  };

  const onDrop = (e: React.DragEvent) => {
    const mediaId = e.dataTransfer.getData(MEDIA_DRAG_TYPE);
    e.preventDefault();
    const pending = drop;
    setDrop(null);
    mediaDrag.current = null;
    if (!mediaId || !pending || (!pending.video && !pending.audio)) return;
    void insertMedia(mediaId, pending.start, { video: pending.video, audio: pending.audio });
  };

  // Commands
  const splitAtPlayhead = useCallback(() => {
    if (!project) return;
    const under = project.sequence.tracks
      .filter((t) => !t.locked)
      .flatMap((t) => t.clips.filter((c) => c.start < playhead && playhead < clipEnd(c)).map((c) => c.id));
    const chosen = selection.length > 0 ? under.filter((id) => selectionSet.has(id)) : under;
    if (chosen.length > 0) void edit({ type: "SplitClips", clipIds: chosen, at: playhead });
  }, [project, playhead, selection, selectionSet, edit]);

  const deleteSelection = useCallback(() => {
    if (selection.length > 0) void edit({ type: "DeleteClips", clipIds: selection });
  }, [selection, edit]);

  const zoomToFit = useCallback(() => {
    if (!project) return;
    const total = Math.max(
      1,
      ...project.sequence.tracks.flatMap((t) => t.clips.map(clipEnd)),
    );
    setZoom((width - 40) / total);
    setScrollX(0);
  }, [project, width, setZoom, setScrollX]);

  useShortcut(
    useCallback(
      (e: KeyboardEvent) => {
        const mod = e.metaKey || e.ctrlKey;
        const fps = Math.round(rate.num / rate.den);
        switch (true) {
          case e.key === "s" && !mod:
          case e.key === "k" && mod:
            splitAtPlayhead();
            return true;
          case e.key === "Backspace" || e.key === "Delete":
            deleteSelection();
            return true;
          case e.key === "ArrowLeft":
            setPlayhead(useEditor.getState().playhead - (e.shiftKey ? fps : 1));
            return true;
          case e.key === "ArrowRight":
            setPlayhead(useEditor.getState().playhead + (e.shiftKey ? fps : 1));
            return true;
          case e.key === "Home":
            setPlayhead(0);
            return true;
          case e.key === "End":
            setPlayhead(project ? Math.max(0, ...project.sequence.tracks.flatMap((t) => t.clips.map(clipEnd))) : 0);
            return true;
          case e.key === "=" || e.key === "+":
            setZoom(useEditor.getState().zoom * 1.5);
            return true;
          case e.key === "-":
            setZoom(useEditor.getState().zoom / 1.5);
            return true;
          case e.key === "\\":
            zoomToFit();
            return true;
          case e.key === "n" && !mod:
            setSnap((v) => !v);
            return true;
          case e.key === "Escape":
            select([]);
            return true;
          case e.key === "a" && mod:
            select(project?.sequence.tracks.flatMap((t) => t.clips.map((c) => c.id)) ?? []);
            return true;
          default:
            return false;
        }
      },
      [rate, splitAtPlayhead, deleteSelection, setPlayhead, setZoom, zoomToFit, select, project],
    ),
  );

  if (!project) return <div className="panel" />;

  return (
    <div className="panel">
      <div className="panel-toolbar">
        <span className="timeline-timecode numeric" aria-label="Playhead">
          {timecode(playhead, rate)}
        </span>
        <button className="btn" onClick={splitAtPlayhead} title="Split at playhead (S)">Split</button>
        <button className="btn" onClick={deleteSelection} disabled={selection.length === 0} title="Delete (⌫)">
          Delete
        </button>
        <span className="toolbar-gap" />
        <button className="btn" onClick={() => void edit({ type: "AddTrack", kind: "video" })}>Add video track</button>
        <button className="btn" onClick={() => void edit({ type: "AddTrack", kind: "audio" })}>Add audio track</button>
        <span style={{ flex: 1 }} />
        <button className="btn" aria-pressed={snap} onClick={() => setSnap(!snap)} title="Snapping (N)">
          Snap {snap ? "on" : "off"}
        </button>
        <button className="btn" onClick={zoomToFit} title="Fit timeline (\)">Fit</button>
        <input
          className="timeline-zoom"
          type="range"
          min={Math.log(ZOOM_MIN)}
          max={Math.log(ZOOM_MAX)}
          step={0.01}
          value={Math.log(zoom)}
          onChange={(e) => setZoom(Math.exp(Number(e.target.value)))}
          aria-label="Zoom"
        />
      </div>
      <div className="panel-body timeline-body">
        <div className="timeline-grid" style={{ height }}>
          <div className="timeline-headers">
            <div className="timeline-ruler-corner" />
            {rows.map((row) => (
              <TrackHeader key={row.track.id} track={row.track} height={row.height} top={row.y} />
            ))}
          </div>
          <div
            ref={areaRef}
            className="timeline-area"
            onWheel={onWheel}
            onDragOver={onDragOver}
            onDragLeave={() => setDrop(null)}
            onDrop={onDrop}
          >
            <canvas
              ref={canvasRef}
              onPointerDown={onPointerDown}
              onPointerMove={onPointerMove}
              onPointerUp={onPointerUp}
              onPointerCancel={() => setDrag(null)}
            />
            {project.sequence.tracks.every((t) => t.clips.length === 0) && !drop && (
              <p className="timeline-hint" style={{ left: frameToX(0, view) + 16 }}>
                Drag media here from the bin, or double-click it to place it at the playhead.
              </p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function TrackHeader({ track, height, top }: { track: Track; height: number; top: number }) {
  const edit = useEditor((s) => s.edit);
  const setState = (patch: { enabled?: boolean; locked?: boolean }) =>
    void edit({
      type: "SetTrackState",
      trackId: track.id,
      enabled: patch.enabled ?? null,
      locked: patch.locked ?? null,
    });
  return (
    <div className={`track-header track-header-${track.kind}`} style={{ height, top }}>
      <span className="track-name">{track.name}</span>
      <button
        className="icon-btn"
        aria-pressed={!track.enabled}
        title={track.kind === "video" ? (track.enabled ? "Hide track" : "Show track") : track.enabled ? "Mute track" : "Unmute track"}
        onClick={() => setState({ enabled: !track.enabled })}
      >
        {track.kind === "video" ? (track.enabled ? <EyeIcon /> : <EyeOffIcon />) : track.enabled ? <SpeakerIcon /> : <MuteIcon />}
      </button>
      <button
        className="icon-btn"
        aria-pressed={track.locked}
        title={track.locked ? "Unlock track" : "Lock track"}
        onClick={() => setState({ locked: !track.locked })}
      >
        <LockIcon open={!track.locked} />
      </button>
      <button
        className="icon-btn track-remove"
        title="Remove track and its clips"
        onClick={() => void edit({ type: "RemoveTrack", trackId: track.id })}
        disabled={track.locked}
      >
        ×
      </button>
    </div>
  );
}

const iconProps = { width: 14, height: 14, viewBox: "0 0 16 16", fill: "none", stroke: "currentColor", strokeWidth: 1.4 } as const;

function EyeIcon() {
  return (
    <svg {...iconProps} aria-hidden>
      <path d="M1.5 8s2.4-4.5 6.5-4.5S14.5 8 14.5 8 12.1 12.5 8 12.5 1.5 8 1.5 8Z" />
      <circle cx="8" cy="8" r="2" />
    </svg>
  );
}

function EyeOffIcon() {
  return (
    <svg {...iconProps} aria-hidden>
      <path d="M1.5 8s2.4-4.5 6.5-4.5S14.5 8 14.5 8 12.1 12.5 8 12.5 1.5 8 1.5 8Z" />
      <path d="M2.5 13.5l11-11" />
    </svg>
  );
}

function SpeakerIcon() {
  return (
    <svg {...iconProps} aria-hidden>
      <path d="M2.5 6h2.5L8.5 3v10L5 10H2.5Z" />
      <path d="M11 5.5a3.5 3.5 0 0 1 0 5" />
    </svg>
  );
}

function MuteIcon() {
  return (
    <svg {...iconProps} aria-hidden>
      <path d="M2.5 6h2.5L8.5 3v10L5 10H2.5Z" />
      <path d="M10.5 6l3.5 4M14 6l-3.5 4" />
    </svg>
  );
}

function LockIcon({ open }: { open: boolean }) {
  return (
    <svg {...iconProps} aria-hidden>
      <rect x="3" y="7.5" width="10" height="6.5" rx="1" />
      <path d={open ? "M5.5 7.5V5a2.5 2.5 0 0 1 4.9-.7" : "M5.5 7.5V5a2.5 2.5 0 0 1 5 0v2.5"} />
    </svg>
  );
}
