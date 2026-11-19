import { useEffect, useMemo, useState } from "react";
import { importMedia } from "../actions";
import type { MediaAsset } from "../bindings/MediaAsset";
import type { MediaId } from "../bindings/MediaId";
import { insertMedia, MEDIA_DRAG_TYPE, mediaDrag } from "../edits";
import { chord } from "../keyboard";
import { formatDuration, formatRate } from "../format";
import { useEditor } from "../store";
import { useTheme } from "../theme";
import { cn } from "../lib/cn";
import { ContextMenu, EmptyState, Icon, IconButton, List, Menu, type MenuEntry, Panel, PanelToolbar, PlainButton, Row, SearchInput, Segmented, TextInput } from "../ui";
import { activeSequence } from "../sequences";
import {
  defaultDirection,
  filterByName,
  nextSort,
  smartBins,
  sortLabels,
  TIMELINES_BIN,
  type BinId,
  type SortDirection,
  type SortKey,
} from "./bins";
import { MediaTile } from "./MediaTile";
import "./media-bin.css";
import type { Sequence } from "../bindings/Sequence";
import { NewSequenceDialog } from "../workspace/NewSequenceDialog";

type ViewMode = "grid" | "list";

const PREFS_KEY = "studio.mediaBin";

interface Prefs {
  bin: BinId;
  view: ViewMode;
  sort: SortKey;
  direction: SortDirection;
}

function loadPrefs(): Prefs {
  try {
    const parsed = JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}") as Partial<Prefs>;
    const sort = parsed.sort && parsed.sort in sortLabels ? parsed.sort : "name";
    return {
      bin: parsed.bin ?? "all",
      view: parsed.view ?? "grid",
      sort,
      // Preferences saved before there was a direction have none; the key's own
      // default reproduces the order they were already seeing.
      direction: parsed.direction ?? defaultDirection(sort),
    };
  } catch {
    return { bin: "all", view: "grid", sort: "name", direction: "asc" };
  }
}

/**
 * A sequence's length in microseconds, so it reads like a clip's duration. Zero rather
 * than null for an empty timeline: null means "a still" to formatDuration, which is
 * not what an empty sequence is.
 */
function sequenceMicros(sequence: Sequence): number {
  const end = Math.max(0, ...sequence.tracks.flatMap((t) => t.clips.map((c) => c.start + c.duration)));
  const { num, den } = sequence.format.frameRate;
  return Math.round((end * den * 1_000_000) / num);
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

/**
 * One sortable column heading. The cell carries `aria-sort` and holds a real button, so
 * a screen reader hears the order and the keyboard can change it. The arrow shows only
 * on the active column: every column drawing one would say nothing about which is in use.
 */
function SortHeading({ column, prefs, onSort, className }: {
  column: SortKey;
  className?: string;
  prefs: Prefs;
  onSort: (key: SortKey) => void;
}) {
  const active = prefs.sort === column;
  return (
    <div
      role="columnheader"
      className={className}
      aria-sort={active ? (prefs.direction === "asc" ? "ascending" : "descending") : "none"}
    >
      <PlainButton className="bin-sort" aria-pressed={active} onClick={() => onSort(column)}>
        {sortLabels[column]}
        {active && <Icon name="chevronDown" className={cn("size-6", prefs.direction === "asc" && "rotate-180")} />}
      </PlainButton>
    </div>
  );
}

export function MediaBinPanel() {
  const theme = useTheme();
  const snapshot = useEditor((s) => s.snapshot);
  const playhead = useEditor((s) => s.playhead);
  const edit = useEditor((s) => s.edit);
  const setExportOpen = useEditor((s) => s.setExportOpen);
  const [selected, setSelected] = useState<MediaId | null>(null);
  const [prefs, setPrefs] = useState<Prefs>(loadPrefs);
  const [newSequence, setNewSequence] = useState(false);
  // Deliberately not in Prefs: a filter is something you are doing now, and reopening
  // the app to find half the bin hidden would read as lost media.
  const [query, setQuery] = useState("");

  useEffect(() => {
    try {
      localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
    } catch {
      // Preferences are a convenience; ignore storage failures.
    }
  }, [prefs]);

  const media = useMemo(() => snapshot?.project.media ?? [], [snapshot]);
  const offline = useMemo(() => new Set(snapshot?.offlineMedia ?? []), [snapshot]);
  const bins = useMemo(() => smartBins(media, offline, prefs.sort, prefs.direction), [media, offline, prefs.sort, prefs.direction]);

  // A check against the active key rather than a separate radio control: the menu is
  // only ever a few mutually exclusive choices. Direction is its own pair so the grid,
  // which has no column headers to click, can still reverse the order.
  const sortEntries: MenuEntry[] = [
    { type: "label", label: "Sort by" },
    ...(Object.keys(sortLabels) as SortKey[]).map((key) => ({
      label: sortLabels[key],
      icon: key === prefs.sort ? ("check" as const) : undefined,
      onSelect: () =>
        setPrefs({
          ...prefs,
          sort: key,
          direction: key === prefs.sort ? prefs.direction : defaultDirection(key),
        }),
    })),
    { type: "separator" },
    ...(["asc", "desc"] as const).map((direction) => ({
      label: direction === "asc" ? "Ascending" : "Descending",
      icon: direction === prefs.direction ? ("check" as const) : undefined,
      onSelect: () => setPrefs({ ...prefs, direction }),
    })),
  ];
  const sortBy = (key: SortKey) => setPrefs({ ...prefs, ...nextSort(prefs, key) });
  const sequences = snapshot?.project.sequences ?? [];
  // Which timeline is being renamed in place, the way the Finder and both reference
  // editors do it: the row's own name turns into the field, so there is no dialog to
  // explain what is being renamed.
  const [renaming, setRenaming] = useState<string | null>(null);
  const activeId = useEditor((st) => st.activeSequenceId);
  const setActiveSequence = useEditor((st) => st.setActiveSequence);
  const openSequenceId = snapshot ? activeSequence(snapshot.project, activeId).id : null;
  const showingTimelines = prefs.bin === TIMELINES_BIN;
  const bin = bins.find((b) => b.id === prefs.bin) ?? bins[0]!;
  const usedIds = new Set(
    snapshot ? snapshot.project.sequences.flatMap((q) => q.tracks.flatMap((t) => t.clips.map((c) => c.source.mediaId))) : [],
  );
  const thumbnail = theme.size.thumbnail;
  const searching = query.trim() !== "";
  const shownSequences = filterByName(sequences, query);
  const shownMedia = filterByName(bin.media, query);
  const noMatches = (
    <EmptyState>{`Nothing in ${showingTimelines ? "timelines" : bin.name.toLowerCase()} matches “${query.trim()}”.`}</EmptyState>
  );

  // Smart bins plus the timelines, as one row of choices. Names only: five of these
  // with an icon each needs more width than the panel has, and the names are already
  // the whole of what a bin is.
  const binOptions = [
    ...bins.map((b) => ({ value: b.id, label: b.name })),
    { value: TIMELINES_BIN, label: "Timelines" },
  ];

  const removeMedia = (id: MediaId) =>
    void edit({ type: "RemoveMedia", mediaId: id }).then(() => setSelected((s) => (s === id ? null : s)));

  const sequenceMenu = (q: Sequence): MenuEntry[] => [
    { label: "Open", icon: "timeline", onSelect: () => setActiveSequence(q.id) },
    { label: "Export…", icon: "export", onSelect: () => { setActiveSequence(q.id); setExportOpen(true); } },
    { label: "Rename", icon: "pencil", onSelect: () => setRenaming(q.id) },
    { type: "separator" },
    {
      label: sequences.length === 1 ? "Delete (the only timeline)" : "Delete",
      icon: "trash",
      tone: "danger",
      disabled: sequences.length === 1,
      onSelect: () => void edit({ type: "RemoveSequence", sequenceId: q.id }),
    },
  ];

  const menuFor = (m: MediaAsset): MenuEntry[] => {
    const used = usedIds.has(m.id);
    return [
      { label: "Add to timeline", icon: "plus", onSelect: () => void insertMedia(m.id, playhead) },
      { type: "separator" },
      {
        label: used ? "Remove (used on the timeline)" : "Remove from project",
        icon: "trash",
        tone: "danger",
        disabled: used,
        onSelect: () => removeMedia(m.id),
      },
    ];
  };

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
      {/* How the media is shown on the left, which media on the right: the view is a
          setting you make once, the bin is the choice you keep making. */}
      <PanelToolbar>
        <Segmented
          emphasis="accent"
          label="Bin"
          value={prefs.bin}
          onChange={(id) => setPrefs({ ...prefs, bin: id as BinId })}
          options={binOptions}
        />
        {showingTimelines && (
          <IconButton
            className="bin-new-timeline"
            icon="plus"
            label="New timeline"
            onClick={() => setNewSequence(true)}
          />
        )}
      </PanelToolbar>

      <div className="bin-content">
            {media.length === 0 && !showingTimelines ? (
              <EmptyState
                action={<IconButton icon="plus" shape="round" label="Import media" shortcut={chord("I")} onClick={importMedia} />}
              />
            ) : showingTimelines && searching && shownSequences.length === 0 ? (
              noMatches
            ) : showingTimelines ? (
              <>
              <div className="bin-header bin-columns bin-table" role="row" aria-hidden>
                <span />
                <span className="bin-heading">Name</span>
                <span className="bin-heading bin-format">Format</span>
                <span className="bin-heading bin-duration">Duration</span>
              </div>
              <List className="bin-list bin-table" label="Timelines">
                  {shownSequences.map((q) => (
                    <ContextMenu key={q.id} entries={sequenceMenu(q)}>
                      <Row
                        selected={q.id === openSequenceId}
                        marker="none"
                        className="bin-row bin-columns"
                        onClick={() => renaming !== q.id && setActiveSequence(q.id)}
                        onDoubleClick={() => setRenaming(q.id)}
                        title={renaming === q.id ? undefined : "Open this timeline"}
                      >
                        <span className="bin-swatch" data-kind="timeline" aria-hidden>
                          <Icon name="timeline" />
                        </span>
                        <span className="bin-name">
                          {renaming === q.id ? (
                            <TextInput
                              focused
                              value={q.name}
                              ariaLabel="Timeline name"
                              onCommit={(name) =>
                                void edit({ type: "RenameSequence", sequenceId: q.id, name })
                              }
                              onDone={() => setRenaming(null)}
                            />
                          ) : (
                            q.name
                          )}
                        </span>
                        <span className="bin-format">
                          {q.format.width}×{q.format.height}, {formatRate(q.format.frameRate)} fps
                        </span>
                        <span className="bin-duration">
                          {formatDuration(sequenceMicros(q))}
                        </span>
                      </Row>
                    </ContextMenu>
                  ))}
              </List>
              </>
            ) : bin.media.length === 0 ? (
              <EmptyState>Nothing in {bin.name.toLowerCase()} yet.</EmptyState>
            ) : shownMedia.length === 0 ? (
              noMatches
            ) : prefs.view === "grid" ? (
              <List
                className="bin-grid"
                label={bin.name}
                layout="grid"
                style={{ gridTemplateColumns: `repeat(auto-fill, minmax(${thumbnail}px, 1fr))` }}
              >
                {shownMedia.map((m) => (
                  <MediaTile
                    key={m.id}
                    media={m}
                    selected={m.id === selected}
                    offline={offline.has(m.id)}
                    onSelect={() => setSelected(m.id)}
                    onPlace={() => void insertMedia(m.id, playhead)}
                    menu={menuFor(m)}
                    {...dragProps(m)}
                  />
                ))}
              </List>
            ) : (
              <>
              <div className="bin-header bin-columns bin-table" role="row">
                {/* The same grid as a row (.bin-columns), so a heading cannot drift off its column. */}
                <span aria-hidden />
                <SortHeading column="name" prefs={prefs} onSort={sortBy} />
                <SortHeading column="format" prefs={prefs} onSort={sortBy} className="bin-format" />
                <SortHeading column="duration" prefs={prefs} onSort={sortBy} className="bin-duration" />
              </div>
              <List className="bin-list bin-table" label={bin.name}>
                  {shownMedia.map((m) => (
                    <ContextMenu key={m.id} entries={menuFor(m)}>
                      <Row
                        selected={m.id === selected}
                        marker="none"
                        className="bin-row bin-columns"
                        draggable
                        onClick={() => setSelected(m.id)}
                        onContextMenu={() => setSelected(m.id)}
                        onDoubleClick={() => void insertMedia(m.id, playhead)}
                        title={m.path}
                        {...dragProps(m)}
                      >
                        <span className="bin-swatch" data-kind={m.kind} aria-hidden>
                          <Icon name={m.kind} />
                        </span>
                        <span className="bin-name">{m.name}</span>
                        <span className="bin-format">{describe(m)}</span>
                        <span className="bin-duration">
                          {offline.has(m.id) ? <em className="bin-offline">Offline</em> : formatDuration(m.durationUs)}
                        </span>
                      </Row>
                    </ContextMenu>
                  ))}
              </List>
              </>
            )}
      </div>

      <NewSequenceDialog open={newSequence} onClose={() => setNewSequence(false)} />

      <PanelToolbar placement="bottom">
        <Segmented
          label="View"
          value={prefs.view}
          onChange={(view) => setPrefs({ ...prefs, view: view as ViewMode })}
          options={[
            { value: "grid", label: "Grid view", icon: "grid", iconOnly: true },
            { value: "list", label: "List view", icon: "list", iconOnly: true },
          ]}
        />
        <SearchInput
          value={query}
          onChange={setQuery}
          ariaLabel="Search this bin"
          placeholder="Search"
        />
        <Menu
          trigger={<IconButton icon="sort" label={`Sort by ${sortLabels[prefs.sort].toLowerCase()}, ${prefs.direction === "asc" ? "ascending" : "descending"}`} />}
          entries={sortEntries}
        />
      </PanelToolbar>
    </Panel>
  );
}
