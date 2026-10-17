import { useEffect, useMemo, useState } from "react";
import { importMedia } from "../actions";
import type { MediaAsset } from "../bindings/MediaAsset";
import type { MediaId } from "../bindings/MediaId";
import { insertMedia, MEDIA_DRAG_TYPE, mediaDrag } from "../edits";
import { formatDuration, formatRate } from "../format";
import { useEditor } from "../store";
import { useTheme } from "../theme";
import { Button, EmptyState, Icon, Panel, PanelToolbar, Segmented, Slider, Spacer, Well } from "../ui";
import { smartBins, type BinId } from "./bins";
import { MediaTile } from "./MediaTile";
import "./media-bin.css";

type ViewMode = "grid" | "list";

const PREFS_KEY = "studio.mediaBin";

interface Prefs {
  bin: BinId;
  view: ViewMode;
  thumbnail: number | null;
}

function loadPrefs(): Prefs {
  try {
    const parsed = JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}") as Partial<Prefs>;
    return { bin: parsed.bin ?? "all", view: parsed.view ?? "grid", thumbnail: parsed.thumbnail ?? null };
  } catch {
    return { bin: "all", view: "grid", thumbnail: null };
  }
}

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
  const theme = useTheme();
  const snapshot = useEditor((s) => s.snapshot);
  const playhead = useEditor((s) => s.playhead);
  const edit = useEditor((s) => s.edit);
  const [selected, setSelected] = useState<MediaId | null>(null);
  const [prefs, setPrefs] = useState<Prefs>(loadPrefs);

  useEffect(() => {
    try {
      localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
    } catch {
      // Preferences are a convenience; ignore storage failures.
    }
  }, [prefs]);

  const media = useMemo(() => snapshot?.project.media ?? [], [snapshot]);
  const offline = useMemo(() => new Set(snapshot?.offlineMedia ?? []), [snapshot]);
  const bins = useMemo(() => smartBins(media, offline), [media, offline]);
  const bin = bins.find((b) => b.id === prefs.bin) ?? bins[0]!;
  const usedIds = new Set(
    snapshot?.project.sequence.tracks.flatMap((t) => t.clips.map((c) => c.source.mediaId)) ?? [],
  );
  const current = media.find((m) => m.id === selected) ?? null;
  const inUse = current !== null && usedIds.has(current.id);
  const thumbnail = Math.min(
    theme.size.thumbnailMax,
    Math.max(theme.size.thumbnailMin, prefs.thumbnail ?? theme.size.thumbnail),
  );

  const dragProps = (m: MediaAsset) => ({
    onDragStart: (e: React.DragEvent) => {
      e.dataTransfer.setData(MEDIA_DRAG_TYPE, m.id);
      e.dataTransfer.effectAllowed = "copy";
      mediaDrag.current = m.id;
      setSelected(m.id);
    },
    onDragEnd: () => {
      mediaDrag.current = null;
    },
  });

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

      {media.length === 0 ? (
        <EmptyState action={<Button variant="solid" onClick={importMedia}>Import media</Button>}>
          Import footage, audio or stills to start cutting.
        </EmptyState>
      ) : (
        <div className="bin-layout">
          <Well className="bin-folders">
            <ul role="listbox" aria-label="Bins">
              {bins.map((b) => (
                <li
                  key={b.id}
                  role="option"
                  aria-selected={b.id === bin.id}
                  className="bin-folder"
                  data-tone={b.id === "offline" ? "danger" : undefined}
                  onClick={() => setPrefs({ ...prefs, bin: b.id })}
                >
                  <Icon name={b.icon} />
                  <span className="bin-folder-name">{b.name}</span>
                  <span className="bin-folder-count numeric">{b.media.length}</span>
                </li>
              ))}
            </ul>
          </Well>

          <div className="bin-content">
            {bin.media.length === 0 ? (
              <EmptyState>Nothing in {bin.name.toLowerCase()} yet.</EmptyState>
            ) : prefs.view === "grid" ? (
              <ul
                className="bin-grid"
                role="listbox"
                aria-label={bin.name}
                style={{ gridTemplateColumns: `repeat(auto-fill, minmax(${thumbnail}px, 1fr))` }}
              >
                {bin.media.map((m) => (
                  <MediaTile
                    key={m.id}
                    media={m}
                    selected={m.id === selected}
                    offline={offline.has(m.id)}
                    onSelect={() => setSelected(m.id)}
                    onPlace={() => void insertMedia(m.id, playhead)}
                    {...dragProps(m)}
                  />
                ))}
              </ul>
            ) : (
              <Well className="bin-list-well">
                <ul className="bin-list" role="listbox" aria-label={bin.name}>
                  {bin.media.map((m) => (
                    <li
                      key={m.id}
                      role="option"
                      aria-selected={m.id === selected}
                      className="bin-row"
                      draggable
                      onClick={() => setSelected(m.id)}
                      onDoubleClick={() => void insertMedia(m.id, playhead)}
                      title={m.path}
                      {...dragProps(m)}
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
              </Well>
            )}
          </div>
        </div>
      )}

      <PanelToolbar placement="bottom">
        <Segmented
          label="View"
          value={prefs.view}
          onChange={(view) => setPrefs({ ...prefs, view: view as ViewMode })}
          options={[
            { value: "grid", label: "Grid view", icon: "grid" },
            { value: "list", label: "List view", icon: "list" },
          ]}
        />
        <Spacer />
        {prefs.view === "grid" && (
          <Slider
            className="bin-size"
            ariaLabel="Thumbnail size"
            min={theme.size.thumbnailMin}
            max={theme.size.thumbnailMax}
            step={1}
            value={thumbnail}
            onChange={(value) => setPrefs({ ...prefs, thumbnail: value })}
          />
        )}
      </PanelToolbar>
    </Panel>
  );
}
