// Canvas painting for the timeline. Reads geometry, never decides behaviour.

import type { Clip } from "../bindings/Clip";
import type { ClipId } from "../bindings/ClipId";
import type { Project } from "../bindings/Project";
import type { TrackId } from "../bindings/TrackId";
import { timecode } from "../format";
import { clipEnd, frameToX, RULER_HEIGHT, tickStep, type Row, type View } from "./geometry";

/** A clip drawn somewhere other than where the document has it (drag preview). */
export interface Ghost {
  trackId: TrackId;
  start: number;
  duration: number;
  name: string;
  kind: "video" | "audio";
}

export interface Scene {
  project: Project;
  rows: Row[];
  view: View;
  width: number;
  height: number;
  playhead: number;
  selection: Set<ClipId>;
  offline: Set<string>;
  /** Clips hidden because a ghost stands in for them. */
  lifted: Set<ClipId>;
  ghosts: Ghost[];
}

// Read once: canvas cannot use CSS variables directly.
function palette() {
  const css = getComputedStyle(document.documentElement);
  const v = (name: string) => css.getPropertyValue(name).trim();
  return {
    bg: v("--ink-0"),
    row: v("--ink-1"),
    rowLocked: v("--ink-2"),
    line: v("--line-soft"),
    text: v("--text"),
    muted: v("--text-muted"),
    faint: v("--text-faint"),
    video: v("--video"),
    videoEdge: v("--video-edge"),
    audio: v("--audio"),
    audioEdge: v("--audio-edge"),
    time: v("--time"),
    danger: v("--danger"),
    font: v("--font-ui"),
  };
}

let cached: ReturnType<typeof palette> | null = null;

export function draw(ctx: CanvasRenderingContext2D, scene: Scene) {
  const c = (cached ??= palette());
  const { view, width, height, rows, project } = scene;
  const rate = project.sequence.format.frameRate;
  const fps = rate.num / rate.den;

  ctx.clearRect(0, 0, width, height);
  ctx.fillStyle = c.bg;
  ctx.fillRect(0, 0, width, height);

  // Rows
  for (const row of rows) {
    ctx.fillStyle = row.track.locked ? c.rowLocked : c.row;
    ctx.fillRect(0, row.y, width, row.height);
  }

  // Ruler and grid
  const step = tickStep(Math.round(fps), view.zoom);
  const firstFrame = Math.floor(view.scrollX / view.zoom / step) * step;
  ctx.font = `11px ${c.font}`;
  ctx.textBaseline = "middle";
  for (let f = firstFrame; frameToX(f, view) < width; f += step) {
    const x = Math.round(frameToX(f, view)) + 0.5;
    ctx.strokeStyle = c.line;
    ctx.beginPath();
    ctx.moveTo(x, RULER_HEIGHT);
    ctx.lineTo(x, height);
    ctx.stroke();
    ctx.strokeStyle = c.faint;
    ctx.beginPath();
    ctx.moveTo(x, RULER_HEIGHT - 8);
    ctx.lineTo(x, RULER_HEIGHT);
    ctx.stroke();
    ctx.fillStyle = c.muted;
    const label = step < Math.round(fps) ? timecode(f, rate) : timecode(f, rate).slice(0, 8);
    ctx.fillText(label, x + 4, RULER_HEIGHT / 2 - 2);
  }
  ctx.fillStyle = c.line;
  ctx.fillRect(0, RULER_HEIGHT - 1, width, 1);

  // Clips
  for (const row of rows) {
    for (const clip of row.track.clips) {
      if (scene.lifted.has(clip.id)) {
        drawClip(ctx, c, scene, row, clip, { alpha: 0.25 });
      } else {
        drawClip(ctx, c, scene, row, clip, {});
      }
    }
  }
  for (const ghost of scene.ghosts) {
    const row = rows.find((r) => r.track.id === ghost.trackId);
    if (!row) continue;
    const fake: Clip = {
      id: "ghost",
      name: ghost.name,
      start: ghost.start,
      duration: ghost.duration,
      enabled: true,
      link: null,
      source: { type: "media", mediaId: "", sourceStart: 0 },
    };
    drawClip(ctx, c, scene, row, fake, { ghost: true });
  }

  // Playhead
  const px = Math.round(frameToX(scene.playhead, view)) + 0.5;
  if (px >= 0 && px <= width) {
    ctx.strokeStyle = c.time;
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(px, 0);
    ctx.lineTo(px, height);
    ctx.stroke();
    ctx.fillStyle = c.time;
    ctx.beginPath();
    ctx.moveTo(px - 6, 0);
    ctx.lineTo(px + 6, 0);
    ctx.lineTo(px + 6, 8);
    ctx.lineTo(px, 14);
    ctx.lineTo(px - 6, 8);
    ctx.closePath();
    ctx.fill();
  }
}

function drawClip(
  ctx: CanvasRenderingContext2D,
  c: ReturnType<typeof palette>,
  scene: Scene,
  row: Row,
  clip: Clip,
  opts: { alpha?: number; ghost?: boolean },
) {
  const x0 = frameToX(clip.start, scene.view);
  const x1 = frameToX(clipEnd(clip), scene.view);
  if (x1 < 0 || x0 > scene.width) return;
  const kind = row.track.kind;
  const y = row.y + 3;
  const h = row.height - 6;
  // One pixel of air between butted clips keeps every cut visible.
  const w = Math.max(1, x1 - x0 - 1);
  const selected = scene.selection.has(clip.id) && !opts.ghost;
  const disabled = !clip.enabled || !row.track.enabled;

  ctx.save();
  ctx.globalAlpha = opts.alpha ?? (disabled ? 0.4 : 1);
  ctx.fillStyle = kind === "video" ? c.video : c.audio;
  roundRect(ctx, x0, y, w, h, 3);
  ctx.fill();

  // A lighter top band separates adjacent clips and carries the name.
  ctx.fillStyle = kind === "video" ? c.videoEdge : c.audioEdge;
  ctx.globalAlpha *= 0.35;
  roundRect(ctx, x0, y, w, 15, 3);
  ctx.fill();
  ctx.globalAlpha = opts.alpha ?? (disabled ? 0.4 : 1);

  if (scene.offline.has(clip.source.mediaId)) {
    ctx.fillStyle = c.danger;
    ctx.fillRect(x0, y, w, 2);
  }

  if (w > 24) {
    ctx.save();
    ctx.beginPath();
    ctx.rect(x0 + 4, y, w - 8, h);
    ctx.clip();
    ctx.fillStyle = "rgba(255,255,255,0.92)";
    ctx.font = `11px ${c.font}`;
    ctx.textBaseline = "middle";
    ctx.fillText(clip.name, x0 + 5, y + 8);
    ctx.restore();
  }

  if (selected || opts.ghost) {
    ctx.globalAlpha = 1;
    ctx.strokeStyle = c.time;
    ctx.lineWidth = opts.ghost ? 1 : 2;
    if (opts.ghost) ctx.setLineDash([4, 3]);
    roundRect(ctx, x0 + 1, y + 1, w - 2, h - 2, 3);
    ctx.stroke();
  }
  ctx.restore();
}

function roundRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  const radius = Math.min(r, w / 2, h / 2);
  ctx.beginPath();
  ctx.roundRect(x, y, w, h, radius);
}
