import { useEffect, useState, type ReactNode } from "react";
import type { Clip } from "../bindings/Clip";
import type { Project } from "../bindings/Project";
import type { Rational } from "../bindings/Rational";
import type { SequenceFormat } from "../bindings/SequenceFormat";
import type { Track } from "../bindings/Track";
import { formatRate, timecode } from "../format";
import { findClip } from "../timeline/geometry";
import { useEditor } from "../store";
import "./inspector.css";

const FRAME_RATES: Rational[] = [
  { num: 24000, den: 1001 },
  { num: 24, den: 1 },
  { num: 25, den: 1 },
  { num: 30000, den: 1001 },
  { num: 30, den: 1 },
  { num: 50, den: 1 },
  { num: 60000, den: 1001 },
  { num: 60, den: 1 },
];

const SIZES: { label: string; width: number; height: number }[] = [
  { label: "HD 1920×1080", width: 1920, height: 1080 },
  { label: "UHD 3840×2160", width: 3840, height: 2160 },
  { label: "HD 1280×720", width: 1280, height: 720 },
  { label: "Vertical 1080×1920", width: 1080, height: 1920 },
  { label: "Square 1080×1080", width: 1080, height: 1080 },
  { label: "Portrait 1080×1350", width: 1080, height: 1350 },
];

export function InspectorPanel() {
  const snapshot = useEditor((s) => s.snapshot);
  const selection = useEditor((s) => s.selection);
  const project = snapshot?.project;
  if (!project) return <div className="panel" />;

  const selected = selection
    .map((id) => findClip(project, id))
    .filter((f): f is { clip: Clip; track: Track } => f !== null);
  // A linked pair reads as one clip in the inspector.
  const links = new Set(selected.map((s) => s.clip.link ?? s.clip.id));

  return (
    <div className="panel">
      <div className="panel-body inspector">
        {selected.length === 0 && <SequenceSettings project={project} />}
        {selected.length > 0 && links.size === 1 && <ClipDetails project={project} entry={selected[0]!} />}
        {links.size > 1 && <MultipleClips count={links.size} ids={selection} />}
      </div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="inspector-section">
      <h2>{title}</h2>
      {children}
    </section>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="inspector-row">
      <span>{label}</span>
      {children}
    </label>
  );
}

/** A text field that commits on Enter or blur and reverts on Escape. */
function CommitInput({
  value,
  onCommit,
  type = "text",
  disabled = false,
  ariaLabel,
}: {
  value: string;
  onCommit: (value: string) => void;
  type?: "text" | "number";
  disabled?: boolean;
  ariaLabel?: string;
}) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  return (
    <input
      className="inspector-input numeric"
      type={type}
      value={draft}
      disabled={disabled}
      aria-label={ariaLabel}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => draft !== value && onCommit(draft)}
      onKeyDown={(e) => {
        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
        if (e.key === "Escape") {
          setDraft(value);
          (e.target as HTMLInputElement).blur();
        }
      }}
    />
  );
}

function ClipDetails({ project, entry }: { project: Project; entry: { clip: Clip; track: Track } }) {
  const edit = useEditor((s) => s.edit);
  const { clip, track } = entry;
  const rate = project.sequence.format.frameRate;
  const media = project.media.find((m) => m.id === clip.source.mediaId);
  const locked = track.locked;

  const toFrames = (text: string) => {
    const value = Math.round(Number(text));
    return Number.isFinite(value) ? value : null;
  };

  return (
    <>
      <Section title="Clip">
        <Row label="Name">
          <CommitInput
            value={clip.name}
            disabled={locked}
            onCommit={(name) => void edit({ type: "RenameClip", clipId: clip.id, name })}
          />
        </Row>
        <Row label="Track">
          <span className="inspector-value">
            {track.name}
            {clip.link ? " + linked" : ""}
          </span>
        </Row>
        <Row label="Enabled">
          <input
            type="checkbox"
            checked={clip.enabled}
            disabled={locked}
            onChange={(e) => void edit({ type: "SetClipsEnabled", clipIds: [clip.id], enabled: e.target.checked })}
          />
        </Row>
      </Section>
      <Section title="Timing">
        <Row label="Start (frames)">
          <CommitInput
            type="number"
            value={String(clip.start)}
            disabled={locked}
            onCommit={(text) => {
              const start = toFrames(text);
              if (start !== null) void edit({ type: "MoveClip", clipId: clip.id, start, trackId: null });
            }}
          />
        </Row>
        <Row label="Duration (frames)">
          <CommitInput
            type="number"
            value={String(clip.duration)}
            disabled={locked}
            onCommit={(text) => {
              const duration = toFrames(text);
              if (duration !== null)
                void edit({ type: "TrimClip", clipId: clip.id, edge: "end", frame: clip.start + duration });
            }}
          />
        </Row>
        <Row label="In">
          <span className="inspector-value numeric">{timecode(clip.start, rate)}</span>
        </Row>
        <Row label="Out">
          <span className="inspector-value numeric">{timecode(clip.start + clip.duration, rate)}</span>
        </Row>
        <Row label="Source in">
          <span className="inspector-value numeric">{timecode(clip.source.sourceStart, rate)}</span>
        </Row>
      </Section>
      {media && (
        <Section title="Source">
          <Row label="File">
            <span className="inspector-value inspector-path" title={media.path}>
              {media.name}
            </span>
          </Row>
          {media.video && (
            <Row label="Picture">
              <span className="inspector-value numeric">
                {media.video.width}×{media.video.height}, {formatRate(media.video.frameRate)} fps, {media.video.codec}
              </span>
            </Row>
          )}
          {media.audio && (
            <Row label="Sound">
              <span className="inspector-value numeric">
                {media.audio.sampleRate} Hz, {media.audio.channels} ch, {media.audio.codec}
              </span>
            </Row>
          )}
        </Section>
      )}
      {clip.link && (
        <div className="inspector-actions">
          <button className="btn" disabled={locked} onClick={() => void edit({ type: "UnlinkClips", clipIds: [clip.id] })}>
            Unlink video and audio
          </button>
        </div>
      )}
    </>
  );
}

function MultipleClips({ count, ids }: { count: number; ids: string[] }) {
  const edit = useEditor((s) => s.edit);
  return (
    <Section title={`${count} clips selected`}>
      <div className="inspector-actions">
        <button className="btn" onClick={() => void edit({ type: "SetClipsEnabled", clipIds: ids, enabled: true })}>
          Enable
        </button>
        <button className="btn" onClick={() => void edit({ type: "SetClipsEnabled", clipIds: ids, enabled: false })}>
          Disable
        </button>
        <button className="btn" onClick={() => void edit({ type: "DeleteClips", clipIds: ids })}>
          Delete
        </button>
      </div>
    </Section>
  );
}

function SequenceSettings({ project }: { project: Project }) {
  const edit = useEditor((s) => s.edit);
  const format = project.sequence.format;
  const hasClips = project.sequence.tracks.some((t) => t.clips.length > 0);
  const apply = (patch: Partial<SequenceFormat>) =>
    void edit({ type: "SetSequenceFormat", format: { ...format, ...patch } });
  const sizeKey = `${format.width}x${format.height}`;
  const preset = SIZES.find((s) => `${s.width}x${s.height}` === sizeKey);

  return (
    <>
      <Section title="Project">
        <Row label="Name">
          <CommitInput value={project.name} onCommit={(name) => void edit({ type: "RenameProject", name })} />
        </Row>
      </Section>
      <Section title="Sequence">
        <Row label="Frame size">
          <select
            className="inspector-input"
            value={preset ? sizeKey : "custom"}
            onChange={(e) => {
              const size = SIZES.find((s) => `${s.width}x${s.height}` === e.target.value);
              if (size) apply({ width: size.width, height: size.height });
            }}
          >
            {SIZES.map((s) => (
              <option key={s.label} value={`${s.width}x${s.height}`}>
                {s.label}
              </option>
            ))}
            {!preset && <option value="custom">Custom</option>}
          </select>
        </Row>
        <Row label="Width">
          <CommitInput
            type="number"
            value={String(format.width)}
            onCommit={(v) => apply({ width: Math.round(Number(v)) })}
          />
        </Row>
        <Row label="Height">
          <CommitInput
            type="number"
            value={String(format.height)}
            onCommit={(v) => apply({ height: Math.round(Number(v)) })}
          />
        </Row>
        <Row label="Frame rate">
          <select
            className="inspector-input"
            value={`${format.frameRate.num}/${format.frameRate.den}`}
            disabled={hasClips}
            onChange={(e) => {
              const [num, den] = e.target.value.split("/").map(Number);
              apply({ frameRate: { num: num!, den: den! } });
            }}
          >
            {FRAME_RATES.map((r) => (
              <option key={`${r.num}/${r.den}`} value={`${r.num}/${r.den}`}>
                {formatRate(r)} fps
              </option>
            ))}
          </select>
        </Row>
        {hasClips && <p className="inspector-note">Frame rate is fixed once the timeline has clips.</p>}
        <Row label="Sample rate">
          <select
            className="inspector-input"
            value={format.sampleRate}
            onChange={(e) => apply({ sampleRate: Number(e.target.value) })}
          >
            <option value={44100}>44.1 kHz</option>
            <option value={48000}>48 kHz</option>
            <option value={96000}>96 kHz</option>
          </select>
        </Row>
      </Section>
      <p className="inspector-note">Select a clip to see its details.</p>
    </>
  );
}
