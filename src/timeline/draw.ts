// Canvas painting for the timeline. Reads geometry and the theme; never decides
// behaviour. Every color, size, alpha, dash and font here comes from the theme, so the
// timeline restyles with the rest of the app.

import type { Clip } from "../bindings/Clip";
import type { ClipId } from "../bindings/ClipId";
import type { Project } from "../bindings/Project";
import type { TrackId } from "../bindings/TrackId";
import { timecode } from "../format";
import { canvasFont, type ColorRoles, type Theme } from "../theme";
import { clipEnd, frameToX, tickStep, type Row, type View } from "./geometry";
import { firstSequence } from "../sequences";

/** A clip drawn somewhere other than where the document has it (drag preview). */
export interface Ghost {
  trackId: TrackId;
  start: number;
  duration: number;
  name: string;
  kind: "video" | "audio";
}

export interface Scene {
  theme: Theme;
  /** Colors of the surface the canvas sits on (normally the "stage" surface). */
  colors: ColorRoles;
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

export function draw(ctx: CanvasRenderingContext2D, scene: Scene) {
  const { theme, view, width, height, rows, project } = scene;
  const c = scene.colors;
  const m = theme.timeline;
  const rate = firstSequence(project).format.frameRate;
  const fps = Math.round(rate.num / rate.den);

  ctx.clearRect(0, 0, width, height);
  ctx.fillStyle = c.surface.base;
  ctx.fillRect(0, 0, width, height);

  // Rows
  for (const row of rows) {
    ctx.fillStyle = row.track.locked ? c.surface.locked : c.surface.panel;
    ctx.fillRect(0, row.y, width, row.height);
  }

  // Ruler and grid
  const line = theme.border.width;
  const step = tickStep(fps, view.zoom, m.tickMinSpacing);
  const firstFrame = Math.floor(view.scrollX / view.zoom / step) * step;
  ctx.font = canvasFont(theme, "canvas");
  ctx.textBaseline = "middle";
  ctx.lineWidth = line;
  for (let f = firstFrame; frameToX(f, view) < width; f += step) {
    const x = Math.round(frameToX(f, view)) + line / 2;
    ctx.strokeStyle = c.border.subtle;
    ctx.beginPath();
    ctx.moveTo(x, m.rulerHeight);
    ctx.lineTo(x, height);
    ctx.stroke();
    ctx.strokeStyle = c.text.tertiary;
    ctx.beginPath();
    ctx.moveTo(x, m.rulerHeight - m.tickLength);
    ctx.lineTo(x, m.rulerHeight);
    ctx.stroke();
    ctx.fillStyle = c.text.secondary;
    const label = step < fps ? timecode(f, rate) : timecode(f, rate).slice(0, 8);
    ctx.fillText(label, x + m.tickLabelInset, (m.rulerHeight - m.tickLength / 2) / 2);
  }
  ctx.fillStyle = c.border.subtle;
  ctx.fillRect(0, m.rulerHeight - line, width, line);

  // Clips
  for (const row of rows) {
    for (const clip of row.track.clips) {
      drawClip(ctx, scene, row, clip, scene.lifted.has(clip.id) ? { alpha: m.alpha.lifted } : {});
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
    drawClip(ctx, scene, row, fake, { ghost: true });
  }

  // Playhead: a line and a pentagon marker in the ruler.
  const px = Math.round(frameToX(scene.playhead, view)) + m.playheadWidth / 2;
  if (px >= 0 && px <= width) {
    const half = m.playheadHead;
    ctx.strokeStyle = c.accent.base;
    ctx.lineWidth = m.playheadWidth;
    ctx.beginPath();
    ctx.moveTo(px, 0);
    ctx.lineTo(px, height);
    ctx.stroke();
    ctx.fillStyle = c.accent.base;
    ctx.beginPath();
    ctx.moveTo(px - half, 0);
    ctx.lineTo(px + half, 0);
    ctx.lineTo(px + half, half * 1.4);
    ctx.lineTo(px, half * 2.4);
    ctx.lineTo(px - half, half * 1.4);
    ctx.closePath();
    ctx.fill();
  }
}

function drawClip(
  ctx: CanvasRenderingContext2D,
  scene: Scene,
  row: Row,
  clip: Clip,
  opts: { alpha?: number; ghost?: boolean },
) {
  const { theme } = scene;
  const c = scene.colors;
  const m = theme.timeline;
  const x0 = frameToX(clip.start, scene.view);
  const x1 = frameToX(clipEnd(clip), scene.view);
  if (x1 < 0 || x0 > scene.width) return;
  const kind = opts.ghost
    ? row.track.kind
    : (scene.project.media.find((media) => media.id === clip.source.mediaId)?.kind ?? row.track.kind);
  const isAudioTrack = row.track.kind === "audio";
  const fill = isAudioTrack ? c.media.audio : kind === "image" ? c.media.image : c.media.video;
  const edge = isAudioTrack ? c.media.audioEdge : kind === "image" ? c.media.imageEdge : c.media.videoEdge;
  const y = row.y + m.clipInset;
  const h = row.height - 2 * m.clipInset;
  // Air between butted clips keeps every cut visible.
  const w = Math.max(1, x1 - x0 - m.clipGap);
  const selected = scene.selection.has(clip.id) && !opts.ghost;
  const disabled = !clip.enabled || !row.track.enabled;
  const baseAlpha = opts.alpha ?? (disabled ? m.alpha.disabled : 1);

  ctx.save();
  ctx.globalAlpha = baseAlpha;
  ctx.fillStyle = fill;
  roundRect(ctx, x0, y, w, h, theme.radius.clip);
  ctx.fill();

  // A lighter band along the top separates adjacent clips and carries the name.
  ctx.fillStyle = edge;
  ctx.globalAlpha = baseAlpha * m.alpha.band;
  roundRect(ctx, x0, y, w, m.clipBand, theme.radius.clip);
  ctx.fill();
  ctx.globalAlpha = baseAlpha;

  if (scene.offline.has(clip.source.mediaId)) {
    ctx.fillStyle = c.status.danger;
    ctx.fillRect(x0, y, w, m.offlineStripe);
  }

  const inset = m.clipLabelInset;
  if (w > inset * 5) {
    ctx.save();
    ctx.beginPath();
    ctx.rect(x0 + inset, y, w - 2 * inset, h);
    ctx.clip();
    ctx.fillStyle = c.text.onMedia;
    ctx.font = canvasFont(theme, "canvas");
    ctx.textBaseline = "middle";
    ctx.fillText(clip.name, x0 + inset, y + m.clipBand / 2);
    ctx.restore();
  }

  if (selected || opts.ghost) {
    const stroke = opts.ghost ? m.ghostWidth : m.selectionWidth;
    ctx.globalAlpha = 1;
    ctx.strokeStyle = c.selection;
    ctx.lineWidth = stroke;
    if (opts.ghost) ctx.setLineDash(m.ghostDash);
    roundRect(ctx, x0 + stroke / 2, y + stroke / 2, w - stroke, h - stroke, theme.radius.clip);
    ctx.stroke();
  }
  ctx.restore();
}

function roundRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  const radius = Math.max(0, Math.min(r, w / 2, h / 2));
  ctx.beginPath();
  ctx.roundRect(x, y, w, h, radius);
}
