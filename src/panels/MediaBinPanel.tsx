import { useState } from "react";
import { importMedia } from "../actions";
import type { MediaAsset } from "../bindings/MediaAsset";
import type { MediaId } from "../bindings/MediaId";
import { insertMedia, MEDIA_DRAG_TYPE, mediaDrag } from "../edits";
import { formatDuration, formatRate } from "../format";
import { useEditor } from "../store";
import { Button, EmptyState, Icon, Panel, PanelBody, PanelToolbar, Spacer } from "../ui";
import "./media-bin.css";

function describe(media: MediaAsset): string {
  if (media.video && media.kind === "video") {
    return `${media.video.width}×${media.video.height}, ${formatRate(media.video.frameRate)} fps`;
  }
  if (media.video) return `${media.video.width}×${media.video.height}`;
  if (media.audio) {
    const channels =
      media.audio.channels === 1 ? "mono" : media.audio.channels === 2 ? "stereo" : `${media.audio.channels} ch`;
    return `${media.audio.sampleRate / 1000} kHz, ${channels}`;
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
  const inUse = current !== null && usedIds.has(current.id);

  return (
    <Panel>
      <PanelToolbar>
        <Button onClick={importMedia}>Import</Button>
        <Button
          disabled={!current}
          onClick={() => current && void insertMedia(current.id, playhead)}
          title="Place at the playhead on V1 and A1"
        >
          Add to timeline
        </Button>
        <Spacer />
        <Button
          disabled={!current || inUse}
          title={inUse ? "Used on the timeline" : "Remove from the project"}
          onClick={() => {
            if (!current) return;
            void edit({ type: "RemoveMedia", mediaId: current.id }).then(() => setSelected(null));
          }}
        >
          Remove
        </Button>
      </PanelToolbar>
      <PanelBody>
        {media.length === 0 ? (
          <EmptyState action={<Button variant="primary" onClick={importMedia}>Import media</Button>}>
            Import footage, audio or stills to start cutting.
          </EmptyState>
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
                <span className="bin-swatch" data-kind={m.kind} aria-hidden>
                  <Icon name={m.kind} />
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
      </PanelBody>
    </Panel>
  );
}
