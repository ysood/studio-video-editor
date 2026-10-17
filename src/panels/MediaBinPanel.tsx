import { useState } from "react";
import { importMedia } from "../actions";
import type { MediaAsset } from "../bindings/MediaAsset";
import type { MediaId } from "../bindings/MediaId";
import { insertMedia, MEDIA_DRAG_TYPE, mediaDrag } from "../edits";
import { formatDuration, formatRate } from "../format";
import { useEditor } from "../store";
import "./media-bin.css";

function describe(media: MediaAsset): string {
  if (media.video && media.kind === "video") {
    return `${media.video.width}×${media.video.height}, ${formatRate(media.video.frameRate)} fps`;
  }
  if (media.video) return `${media.video.width}×${media.video.height}`;
  if (media.audio) {
    return `${media.audio.sampleRate / 1000} kHz, ${media.audio.channels === 1 ? "mono" : media.audio.channels === 2 ? "stereo" : `${media.audio.channels} ch`}`;
  }
  return "";
}

export function MediaBinPanel() {
  const snapshot = useEditor((s) => s.snapshot);
  const playhead = useEditor((s) => s.playhead);
  const edit = useEditor((s) => s.edit);
  const [selected, setSelected] = useState<MediaId | null>(null);

  const media = snapshot?.project.media ?? [];
  const offline = new Set(snapshot?.offlineMedia ?? []);
  const usedIds = new Set(
    snapshot?.project.sequence.tracks.flatMap((t) => t.clips.map((c) => c.source.mediaId)) ?? [],
  );
  const current = media.find((m) => m.id === selected) ?? null;

  return (
    <div className="panel">
      <div className="panel-toolbar">
        <button className="btn" onClick={importMedia}>Import</button>
        <button
          className="btn"
          disabled={!current}
          onClick={() => current && void insertMedia(current.id, playhead)}
          title="Place at the playhead on V1 and A1"
        >
          Add to timeline
        </button>
        <span style={{ flex: 1 }} />
        <button
          className="btn"
          disabled={!current || usedIds.has(current.id)}
          title={current && usedIds.has(current.id) ? "Used on the timeline" : "Remove from the project"}
          onClick={() => {
            if (!current) return;
            void edit({ type: "RemoveMedia", mediaId: current.id }).then(() => setSelected(null));
          }}
        >
          Remove
        </button>
      </div>
      <div className="panel-body">
        {media.length === 0 ? (
          <div className="empty">
            <p>Import footage, audio or stills to start cutting.</p>
            <div>
              <button className="btn btn-primary" onClick={importMedia}>Import media</button>
            </div>
          </div>
        ) : (
          <ul className="bin-list" role="listbox" aria-label="Media">
            {media.map((m) => (
              <li
                key={m.id}
                role="option"
                aria-selected={m.id === selected}
                className="bin-row"
                draggable
                onDragStart={(e) => {
                  e.dataTransfer.setData(MEDIA_DRAG_TYPE, m.id);
                  e.dataTransfer.effectAllowed = "copy";
                  mediaDrag.current = m.id;
                  setSelected(m.id);
                }}
                onDragEnd={() => {
                  mediaDrag.current = null;
                }}
                onClick={() => setSelected(m.id)}
                onDoubleClick={() => void insertMedia(m.id, playhead)}
                title={m.path}
              >
                <span className={`bin-swatch bin-swatch-${m.kind}`} aria-hidden>
                  {m.kind === "audio" ? "♪" : m.kind === "image" ? "▣" : "▶"}
                </span>
                <span className="bin-name">{m.name}</span>
                <span className="bin-meta numeric">{describe(m)}</span>
                <span className="bin-duration numeric">
                  {offline.has(m.id) ? <em className="bin-offline">Offline</em> : formatDuration(m.durationUs)}
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
