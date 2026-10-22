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
 * A gallery tile: picture on top, caption plate below. Video tiles scrub as the pointer
 * moves across them. Thumbnails come from the media itself for now; a cached thumbnail
 * service in Rust can replace this for large bins.
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
        </div>
        <div className="bin-tile-caption" data-surface="well">
          <span className="bin-tile-name">{media.name}</span>
          <span className="bin-tile-meta numeric">
            <Icon name={media.kind} />
            {formatDuration(media.durationUs)}
          </span>
        </div>
      </Row>
    </ContextMenu>
  );
}
