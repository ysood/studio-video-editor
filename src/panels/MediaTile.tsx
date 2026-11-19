import { convertFileSrc } from "@tauri-apps/api/core";
import { useRef } from "react";
import type { MediaAsset } from "../bindings/MediaAsset";
import { formatDuration } from "../format";
import { ContextMenu, Icon, type MenuEntry, Row } from "../ui";

/** Poster frame position, a little in so fades from black still show picture. */
const POSTER_SECONDS = 0.5;

interface MediaTileProps {
  media: MediaAsset;
  selected: boolean;
  offline: boolean;
  onSelect: () => void;
  onPlace: () => void;
  onDragStart: (event: React.DragEvent) => void;
  onDragEnd: () => void;
  /** Right-click menu; opening it selects the tile. */
  menu: MenuEntry[];
}

/**
 * A gallery tile: the picture and nothing else. Video tiles scrub as the pointer moves
 * across them, with a line marking where in the clip the frame on show comes from.
 *
 * The name is the tile's `aria-label` rather than text under it, so dropping the caption
 * costs the picture nothing and still leaves the tile something to be called.
 *
 * Thumbnails come from the media itself for now; a cached thumbnail service in Rust can
 * replace this for large bins.
 */
export function MediaTile({
  media,
  selected,
  offline,
  onSelect,
  onPlace,
  onDragStart,
  onDragEnd,
  menu,
}: MediaTileProps) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const url = offline ? "" : convertFileSrc(media.path);

  const scrub = (event: React.MouseEvent<HTMLDivElement>) => {
    const video = videoRef.current;
    if (!video || !Number.isFinite(video.duration)) return;
    const rect = event.currentTarget.getBoundingClientRect();
    const fraction = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
    video.currentTime = fraction * video.duration;
    // Written straight to the element: this fires on every pointer move, and a render
    // per move to place a one pixel line would be a poor trade.
    event.currentTarget.style.setProperty("--scrub", String(fraction));
  };

  const resetScrub = () => {
    if (videoRef.current) videoRef.current.currentTime = POSTER_SECONDS;
  };

  return (
    <ContextMenu entries={menu}>
      <Row
        className="bin-tile"
        marker="outline"
        selected={selected}
        draggable
        aria-label={media.name}
        title={media.path}
        onClick={onSelect}
        onContextMenu={onSelect}
        onDoubleClick={onPlace}
        onDragStart={onDragStart}
        onDragEnd={onDragEnd}
      >
        <div
          className="bin-tile-picture"
          data-kind={media.kind}
          onMouseMove={media.kind === "video" ? scrub : undefined}
          onMouseLeave={media.kind === "video" ? resetScrub : undefined}
        >
          {!offline && media.kind === "video" && (
            <video ref={videoRef} src={`${url}#t=${POSTER_SECONDS}`} muted preload="metadata" playsInline />
          )}
          {!offline && media.kind === "image" && <img src={url} alt="" draggable={false} />}
          {(offline || media.kind === "audio") && (
            <span className="bin-tile-glyph" aria-hidden>
              <Icon name={offline ? "warning" : "audio"} />
            </span>
          )}
          {offline && <span className="bin-tile-badge">Offline</span>}
          {!offline && media.kind === "video" && <span className="bin-tile-scrub" aria-hidden />}
        </div>
        {/* A thumbnail does not say which clip it is: two takes of one setup are the same
            picture. The name is what a person reads a bin for, so it is on the tile and
            not only in a tooltip, with the duration beside it as both reference editors
            have it. */}
        <div className="bin-tile-caption">
          <span className="bin-tile-name">{media.name}</span>
          <span className="bin-tile-duration">{formatDuration(media.durationUs)}</span>
        </div>
      </Row>
    </ContextMenu>
  );
}
