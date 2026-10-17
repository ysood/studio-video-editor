import type { IDockviewPanelProps } from "dockview-react";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { ClipEdge } from "../bindings/ClipEdge";
import type { ClipId } from "../bindings/ClipId";
import type { Project } from "../bindings/Project";
import type { Track } from "../bindings/Track";
import type { TrackKind } from "../bindings/TrackKind";
import type { TrackChoice } from "../bindings/TrackChoice";
import type { TrackId } from "../bindings/TrackId";
import { insertMedia, MEDIA_DRAG_TYPE, mediaDrag, trackChoice } from "../edits";
import { microsToFrames } from "../format";
import { chord, useShortcut } from "../keyboard";
import { useEditor, ZOOM_MAX, ZOOM_MIN, useActiveSequence } from "../store";
import { surfaceColors, useTheme, useThemeRevision } from "../theme";
import { draw, type Ghost } from "../timeline/draw";
import {
  clipEnd,
  contentHeight,
  findClip,
  frameToX,
  hitTest,
  layoutRows,
  linkGroup,
  naturalHeight,
  previewMove,
  previewTrim,
  rowAt,
  xToFrame,
  type View,
} from "../timeline/geometry";
import { ContextMenu, Gap, IconButton, Menu, type MenuEntry, Panel, PanelBody, PanelToolbar, Slider, Spacer, Timecode } from "../ui";
import "./timeline.css";
import type { Sequence } from "../bindings/Sequence";

/** Pointer travel before a press on a clip becomes a drag, so clicks stay clicks. */
const DRAG_THRESHOLD_PX = 3;

type Drag =
  | { kind: "scrub" }
  | { kind: "move"; clipId: ClipId; grabOffset: number; startX: number; active: boolean; start: number; trackId: TrackId }
  | { kind: "trim"; clipId: ClipId; edge: ClipEdge; frame: number };

interface DropPreview {
  ghosts: Ghost[];
  start: number;
  video: TrackChoice;
  audio: TrackChoice;
}

function mediaLength(project: Project, sequence: Sequence, mediaId: string): number {
  const media = project.media.find((m) => m.id === mediaId);
  const rate = sequence.format.frameRate;
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
  // The drawing area's own height, which the rows are centred in.
  const [areaHeight, setAreaHeight] = useState(0);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [drop, setDrop] = useState<DropPreview | null>(null);
  const [snap, setSnap] = useState(true);
  const [menuOnClip, setMenuOnClip] = useState(false);

  const theme = useTheme();
  const themeRevision = useThemeRevision();
  const metrics = theme.timeline;
  const project = snapshot?.project ?? null;
  const sequence = useActiveSequence();
  const rows = useMemo(
    () => (sequence ? layoutRows(sequence, metrics, areaHeight) : []),
    [sequence, metrics, areaHeight],
  );
  // The grid is sized by what the rows need, so the panel body can stretch it; the canvas
  // is sized by what it ends up with, so the centred rows have a background either side.
  const gridHeight = sequence ? naturalHeight(sequence, metrics) : metrics.rulerHeight;
  const height = Math.max(areaHeight, contentHeight(rows, metrics));
  const view: View = useMemo(() => ({ zoom, scrollX }), [zoom, scrollX]);
  const rate = sequence?.format.frameRate ?? { num: 24, den: 1 };
  const selectionSet = useMemo(() => new Set(selection), [selection]);

  // Keep the canvas sized to its container. The area only exists once a project has
  // loaded, so the observer attaches then.
  const hasProject = project !== null;
  useLayoutEffect(() => {
    const area = areaRef.current;
    if (!area) return;
    // ResizeObserver alone missed Dockview's initial layout in WebKitGTK, so panel
    // dimension events also trigger a measure.
    const measure = () => {
      setWidth(area.clientWidth);
      setAreaHeight(area.clientHeight);
    };
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
    const margin = theme.space[9];
    if (x > width - margin || x < 0) setScrollX(playhead * zoom - margin);
  }, [playing, playhead, view, width, zoom, setScrollX, theme]);

  // Ghosts for an in-progress move or trim.
  const { ghosts, lifted } = useMemo(() => {
    const ghosts: Ghost[] = [];
    const lifted = new Set<ClipId>();
    if (!project || !sequence || !drag || drag.kind === "scrub") return { ghosts, lifted };
    if (drag.kind === "move" && drag.active) {
      const dragged = findClip(sequence, drag.clipId);
      if (!dragged) return { ghosts, lifted };
      const delta = drag.start - dragged.clip.start;
      for (const id of linkGroup(sequence, drag.clipId)) {
        const found = findClip(sequence, id);
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
      for (const id of linkGroup(sequence, drag.clipId)) {
        const found = findClip(sequence, id);
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
    if (!canvas || !project || !sequence) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.floor(width * dpr);
    canvas.height = Math.floor(height * dpr);
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    draw(ctx, {
      theme,
      colors: surfaceColors(theme, "stage"),
      project,
      sequence,
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
  }, [theme, themeRevision, project, rows, view, width, height, playhead, selectionSet, ghosts, lifted, drop, snapshot]);

  const pointer = (e: { clientX: number; clientY: number }) => {
    const rect = canvasRef.current!.getBoundingClientRect();
    return { x: e.clientX - rect.left, y: e.clientY - rect.top };
  };

  const snapThreshold = metrics.snap / zoom;

  // The canvas is only as tall as the ruler plus the tracks, so a click in the dead
  // space below the last track lands on the area around it and never reaches the
  // handler above. Treat that space as "nothing selected", which brings the sequence
  // settings back in the inspector.
  const onAreaPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.target === e.currentTarget && e.button === 0) select([]);
  };

  const onPointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (!project || !sequence || e.button !== 0) return;
    const { x, y } = pointer(e);
    const hit = hitTest(rows, view, x, y, metrics);
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
    const picked = e.altKey ? [hit.clip.id] : linkGroup(sequence, hit.clip.id);
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

  // Runs before the context menu opens: a right click on a clip selects it (and its
  // link group) unless it is already part of the selection, like Finder and Resolve.
  const onContextMenu = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!project || !sequence) return;
    const { x, y } = pointer(e);
    const hit = hitTest(rows, view, x, y, metrics);
    setMenuOnClip(hit.type === "clip");
    if (hit.type === "clip" && !selectionSet.has(hit.clip.id)) select(linkGroup(sequence, hit.clip.id));
  };

  const onPointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (!project || !sequence) return;
    const { x, y } = pointer(e);
    if (!drag) {
      const hit = hitTest(rows, view, x, y, metrics);
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
      const preview = previewMove(sequence, drag.clipId, xToFrame(x, view) - drag.grabOffset, over, {
        enabled: snap,
        playhead,
        thresholdFrames: snapThreshold,
      });
      if (preview) setDrag({ ...drag, active: true, start: preview.start, trackId: preview.trackId });
    } else if (drag.kind === "trim") {
      let raw = xToFrame(x, view);
      if (snap && Math.abs(raw - playhead) <= snapThreshold) raw = playhead;
      const frame = previewTrim(project, sequence, drag.clipId, drag.edge, raw);
      if (frame !== null) setDrag({ ...drag, frame });
    }
  };

  const onPointerUp = () => {
    if (!project || !sequence || !drag) return;
    setDrag(null);
    if (drag.kind === "move" && drag.active) {
      const found = findClip(sequence, drag.clipId);
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
    if (!project || !sequence) return null;
    const media = project.media.find((m) => m.id === mediaId);
    if (!media) return null;
    const over = rowAt(rows, y)?.track;
    // The track under the cursor claims its own kind; the other stream, and a locked
    // track under the cursor, fall back to the first free track of that kind.
    const usable = (t: Track | undefined) => (t && !t.locked ? t.id : null);
    const named = (kind: TrackKind) => (over?.kind === kind ? usable(over) : null);
    const video = trackChoice(sequence, "video", media.video !== null, named("video"));
    const audio = trackChoice(sequence, "audio", media.audio !== null, named("audio"));
    return { media, video, audio };
  };

  const onDragOver = (e: React.DragEvent) => {
    if (!project || !sequence || !e.dataTransfer.types.includes(MEDIA_DRAG_TYPE) || !mediaDrag.current) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
    const { x, y } = pointer(e);
    const targets = dropTargets(y, mediaDrag.current);
    if (!targets) return;
    const duration = mediaLength(project, sequence, targets.media.id);
    let start = Math.max(0, Math.round(xToFrame(x, view)));
    if (snap) {
      const near = [0, playhead, ...sequence.tracks.flatMap((t) => t.clips.flatMap((c) => [c.start, clipEnd(c)]))];
      const hit = near.find((f) => Math.abs(f - start) <= snapThreshold);
      if (hit !== undefined) start = hit;
    }
    // A ghost for every stream being placed, including one on a track that does not
    // exist yet, so a drop that has to add a track shows the row it will add.
    const ghosts: Ghost[] = [];
    for (const [kind, choice] of [["video", targets.video], ["audio", targets.audio]] as const) {
      if (choice.type === "skip") continue;
      const trackId = choice.type === "track" ? choice.trackId : null;
      ghosts.push({ trackId, start, duration, name: targets.media.name, kind });
    }
    setDrop({ ghosts, start, video: targets.video, audio: targets.audio });
  };

  const onDrop = (e: React.DragEvent) => {
    const mediaId = e.dataTransfer.getData(MEDIA_DRAG_TYPE);
    e.preventDefault();
    const pending = drop;
    setDrop(null);
    mediaDrag.current = null;
    if (!mediaId || !pending) return;
    if (pending.video.type === "skip" && pending.audio.type === "skip") return;
    void insertMedia(mediaId, pending.start, { video: pending.video, audio: pending.audio });
  };

  // Commands
  const splitAtPlayhead = useCallback(() => {
    if (!project || !sequence) return;
    const under = sequence.tracks
      .filter((t) => !t.locked)
      .flatMap((t) => t.clips.filter((c) => c.start < playhead && playhead < clipEnd(c)).map((c) => c.id));
    const chosen = selection.length > 0 ? under.filter((id) => selectionSet.has(id)) : under;
    if (chosen.length > 0) void edit({ type: "SplitClips", clipIds: chosen, at: playhead });
  }, [project, playhead, selection, selectionSet, edit]);

  const deleteSelection = useCallback(() => {
    if (selection.length > 0) void edit({ type: "DeleteClips", clipIds: selection });
  }, [selection, edit]);

  const zoomToFit = useCallback(() => {
    if (!project || !sequence) return;
    const total = Math.max(
      1,
      ...sequence.tracks.flatMap((t) => t.clips.map(clipEnd)),
    );
    setZoom((width - 2 * theme.space[8]) / total);
    setScrollX(0);
  }, [project, width, setZoom, setScrollX, theme]);

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
            setPlayhead(sequence ? Math.max(0, ...sequence.tracks.flatMap((t) => t.clips.map(clipEnd))) : 0);
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
            select(sequence ? sequence.tracks.flatMap((t) => t.clips.map((c) => c.id)) : []);
            return true;
          default:
            return false;
        }
      },
      [rate, splitAtPlayhead, deleteSelection, setPlayhead, setZoom, zoomToFit, select, project],
    ),
  );

  if (!project || !sequence) return <Panel surface="stage" />;

  const selectedClips = sequence.tracks.flatMap((t) => t.clips.filter((c) => selectionSet.has(c.id)));
  const sequenceId = sequence.id;
  const addTrackEntries: MenuEntry[] = [
    { label: "Video track", icon: "video", onSelect: () => void edit({ type: "AddTrack", sequenceId, kind: "video" }) },
    { label: "Audio track", icon: "audio", onSelect: () => void edit({ type: "AddTrack", sequenceId, kind: "audio" }) },
  ];
  const contextEntries: MenuEntry[] =
    menuOnClip && selectedClips.length > 0
      ? clipMenu(selectedClips, playhead, {
          split: splitAtPlayhead,
          remove: deleteSelection,
          setEnabled: (enabled) => void edit({ type: "SetClipsEnabled", clipIds: selection, enabled }),
          unlink: () => void edit({ type: "UnlinkClips", clipIds: selection }),
        })
      : [
          { type: "label", label: "Add" },
          ...addTrackEntries,
          { type: "separator" },
          { label: snap ? "Turn snapping off" : "Turn snapping on", icon: "magnet", shortcut: "N", onSelect: () => setSnap(!snap) },
          { label: "Fit timeline", icon: "fit", shortcut: "\\", onSelect: zoomToFit },
          {
            label: "Select all",
            shortcut: chord("A"),
            onSelect: () => select(sequence.tracks.flatMap((t) => t.clips.map((c) => c.id))),
          },
        ];

  return (
    <Panel surface="stage">
      <PanelToolbar>
        <Timecode frame={playhead} rate={rate} label="Playhead" />
        <Gap />
        <IconButton icon="scissors" label="Split at playhead" shortcut="S" onClick={splitAtPlayhead} />
        <IconButton
          icon="trash"
          label="Delete selection"
          shortcut="⌫"
          onClick={deleteSelection}
          disabled={selection.length === 0}
        />
        <Menu trigger={<IconButton icon="plus" label="Add track" />} entries={addTrackEntries} />
        <Spacer />
        <IconButton icon="magnet" label={snap ? "Snapping on" : "Snapping off"} shortcut="N" pressed={snap} onClick={() => setSnap(!snap)} />
        <IconButton icon="fit" label="Fit timeline" shortcut="\" onClick={zoomToFit} />
        <Slider
          className="timeline-zoom"
          min={Math.log(ZOOM_MIN)}
          max={Math.log(ZOOM_MAX)}
          step={0.01}
          value={Math.log(zoom)}
          onChange={(v) => setZoom(Math.exp(v))}
          ariaLabel="Zoom"
        />
      </PanelToolbar>
      <PanelBody className="timeline-body">
        <div className="timeline-grid" style={{ height: gridHeight }}>
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
            onPointerDown={onAreaPointerDown}
            onDragOver={onDragOver}
            onDragLeave={() => setDrop(null)}
            onDrop={onDrop}
          >
            <ContextMenu entries={contextEntries}>
              <canvas
                ref={canvasRef}
                onContextMenu={onContextMenu}
                onPointerDown={onPointerDown}
                onPointerMove={onPointerMove}
                onPointerUp={onPointerUp}
                onPointerCancel={() => setDrag(null)}
              />
            </ContextMenu>
          </div>
        </div>
      </PanelBody>
    </Panel>
  );
}

type Clip = Project["sequences"][number]["tracks"][number]["clips"][number];

/** Right-click menu for the selected clips. Labels follow the selection's current state. */
function clipMenu(
  clips: Clip[],
  playhead: number,
  actions: { split: () => void; remove: () => void; setEnabled: (enabled: boolean) => void; unlink: () => void },
): MenuEntry[] {
  const allEnabled = clips.every((c) => c.enabled);
  const plural = clips.length > 1 ? "clips" : "clip";
  return [
    {
      label: "Split at playhead",
      icon: "scissors",
      shortcut: "S",
      disabled: !clips.some((c) => c.start < playhead && playhead < clipEnd(c)),
      onSelect: actions.split,
    },
    {
      label: allEnabled ? `Disable ${plural}` : `Enable ${plural}`,
      icon: allEnabled ? "eyeOff" : "eye",
      onSelect: () => actions.setEnabled(!allEnabled),
    },
    { label: "Unlink", icon: "unlink", disabled: !clips.some((c) => c.link !== null), onSelect: actions.unlink },
    { type: "separator" },
    { label: `Delete ${plural}`, icon: "trash", shortcut: "⌫", tone: "danger", onSelect: actions.remove },
  ];
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
  const isVideo = track.kind === "video";
  const entries: MenuEntry[] = [
    {
      label: isVideo ? (track.enabled ? "Hide track" : "Show track") : track.enabled ? "Mute track" : "Unmute track",
      icon: isVideo ? (track.enabled ? "eyeOff" : "eye") : track.enabled ? "mute" : "speaker",
      onSelect: () => setState({ enabled: !track.enabled }),
    },
    {
      label: track.locked ? "Unlock track" : "Lock track",
      icon: track.locked ? "unlock" : "lock",
      onSelect: () => setState({ locked: !track.locked }),
    },
    { type: "separator" },
    {
      label: "Remove track and its clips",
      icon: "trash",
      tone: "danger",
      disabled: track.locked,
      onSelect: () => void edit({ type: "RemoveTrack", trackId: track.id }),
    },
  ];
  return (
    <ContextMenu entries={entries}>
      <div className="track-header" data-kind={track.kind} style={{ height, top }}>
        <span className="track-name">{track.name}</span>
        <IconButton
          size="small"
          icon={isVideo ? (track.enabled ? "eye" : "eyeOff") : track.enabled ? "speaker" : "mute"}
          pressed={!track.enabled}
          label={isVideo ? (track.enabled ? "Hide track" : "Show track") : track.enabled ? "Mute track" : "Unmute track"}
          onClick={() => setState({ enabled: !track.enabled })}
        />
        <IconButton
          size="small"
          icon={track.locked ? "lock" : "unlock"}
          pressed={track.locked}
          label={track.locked ? "Unlock track" : "Lock track"}
          onClick={() => setState({ locked: !track.locked })}
        />
        <IconButton
          size="small"
          icon="close"
          className="track-remove"
          label="Remove track and its clips"
          onClick={() => void edit({ type: "RemoveTrack", trackId: track.id })}
          disabled={track.locked}
        />
      </div>
    </ContextMenu>
  );
}
