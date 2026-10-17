import type { Clip } from "../bindings/Clip";
import type { Project } from "../bindings/Project";
import type { Rational } from "../bindings/Rational";
import type { SequenceFormat } from "../bindings/SequenceFormat";
import type { Track } from "../bindings/Track";
import { formatRate, timecode } from "../format";
import { findClip } from "../timeline/geometry";
import { useEditor } from "../store";
import {
  Actions,
  Button,
  Checkbox,
  Field,
  Note,
  NumberInput,
  Panel,
  PanelBody,
  Section,
  Select,
  TextInput,
  Value,
} from "../ui";
import "./inspector.css";
import { firstSequence } from "../sequences";

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
  if (!project) return <Panel />;

  const selected = selection
    .map((id) => findClip(project, id))
    .filter((f): f is { clip: Clip; track: Track } => f !== null);
  // A linked pair reads as one clip in the inspector.
  const links = new Set(selected.map((s) => s.clip.link ?? s.clip.id));

  return (
    <Panel>
      <PanelBody layout="sheet">
        {selected.length === 0 && <SequenceSettings project={project} />}
        {selected.length > 0 && links.size === 1 && <ClipDetails project={project} entry={selected[0]!} />}
        {links.size > 1 && <MultipleClips count={links.size} ids={selection} />}
      </PanelBody>
    </Panel>
  );
}

function ClipDetails({ project, entry }: { project: Project; entry: { clip: Clip; track: Track } }) {
  const edit = useEditor((s) => s.edit);
  const { clip, track } = entry;
  const rate = firstSequence(project).format.frameRate;
  const media = project.media.find((m) => m.id === clip.source.mediaId);
  const locked = track.locked;

  return (
    <>
      <Section title="Clip">
        <Field label="Name">
          <TextInput
            value={clip.name}
            disabled={locked}
            onCommit={(name) => void edit({ type: "RenameClip", clipId: clip.id, name })}
          />
        </Field>
        <Field label="Track">
          <Value>
            {track.name}
            {clip.link ? " + linked" : ""}
          </Value>
        </Field>
        <Field label="Enabled">
          <Checkbox
            checked={clip.enabled}
            disabled={locked}
            onChange={(enabled) => void edit({ type: "SetClipsEnabled", clipIds: [clip.id], enabled })}
          />
        </Field>
      </Section>
      <Section title="Timing">
        <Field label="Start (frames)">
          <NumberInput
            value={clip.start}
            disabled={locked}
            onCommit={(start) => void edit({ type: "MoveClip", clipId: clip.id, start, trackId: null })}
          />
        </Field>
        <Field label="Duration (frames)">
          <NumberInput
            value={clip.duration}
            disabled={locked}
            onCommit={(duration) =>
              void edit({ type: "TrimClip", clipId: clip.id, edge: "end", frame: clip.start + duration })
            }
          />
        </Field>
        <Field label="In">
          <Value numeric>{timecode(clip.start, rate)}</Value>
        </Field>
        <Field label="Out">
          <Value numeric>{timecode(clip.start + clip.duration, rate)}</Value>
        </Field>
        <Field label="Source in">
          <Value numeric>{timecode(clip.source.sourceStart, rate)}</Value>
        </Field>
      </Section>
      {media && (
        <Section title="Source">
          <Field label="File">
            <Value truncate="start" title={media.path}>
              {media.name}
            </Value>
          </Field>
          {media.video && (
            <Field label="Picture">
              <Value numeric>
                {media.video.width}×{media.video.height}, {formatRate(media.video.frameRate)} fps, {media.video.codec}
              </Value>
            </Field>
          )}
          {media.audio && (
            <Field label="Sound">
              <Value numeric>
                {media.audio.sampleRate} Hz, {media.audio.channels} ch, {media.audio.codec}
              </Value>
            </Field>
          )}
        </Section>
      )}
      {clip.link && (
        <Actions>
          <Button disabled={locked} onClick={() => void edit({ type: "UnlinkClips", clipIds: [clip.id] })}>
            Unlink video and audio
          </Button>
        </Actions>
      )}
    </>
  );
}

function MultipleClips({ count, ids }: { count: number; ids: string[] }) {
  const edit = useEditor((s) => s.edit);
  return (
    <Section title={`${count} clips selected`}>
      <Actions>
        <Button onClick={() => void edit({ type: "SetClipsEnabled", clipIds: ids, enabled: true })}>Enable</Button>
        <Button onClick={() => void edit({ type: "SetClipsEnabled", clipIds: ids, enabled: false })}>Disable</Button>
        <Button variant="ghost" onClick={() => void edit({ type: "DeleteClips", clipIds: ids })}>
          Delete
        </Button>
      </Actions>
    </Section>
  );
}

function SequenceSettings({ project }: { project: Project }) {
  const edit = useEditor((s) => s.edit);
  const format = firstSequence(project).format;
  const hasClips = firstSequence(project).tracks.some((t) => t.clips.length > 0);
  const apply = (patch: Partial<SequenceFormat>) =>
    void edit({
      type: "SetSequenceFormat",
      sequenceId: firstSequence(project).id,
      format: { ...format, ...patch },
    });
  const sizeKey = `${format.width}x${format.height}`;
  const preset = SIZES.find((s) => `${s.width}x${s.height}` === sizeKey);
  const sizeOptions = SIZES.map((s) => ({ value: `${s.width}x${s.height}`, label: s.label }));
  if (!preset) sizeOptions.push({ value: "custom", label: "Custom" });

  return (
    <>
      <Section title="Project">
        <Field label="Name">
          <TextInput value={project.name} onCommit={(name) => void edit({ type: "RenameProject", name })} />
        </Field>
      </Section>
      <Section title="Sequence">
        <Field label="Frame size">
          <Select
            value={preset ? sizeKey : "custom"}
            options={sizeOptions}
            onChange={(value) => {
              const size = SIZES.find((s) => `${s.width}x${s.height}` === value);
              if (size) apply({ width: size.width, height: size.height });
            }}
          />
        </Field>
        <Field label="Width">
          <NumberInput value={format.width} onCommit={(width) => apply({ width })} />
        </Field>
        <Field label="Height">
          <NumberInput value={format.height} onCommit={(height) => apply({ height })} />
        </Field>
        <Field label="Frame rate">
          <Select
            value={`${format.frameRate.num}/${format.frameRate.den}`}
            disabled={hasClips}
            options={FRAME_RATES.map((r) => ({ value: `${r.num}/${r.den}`, label: `${formatRate(r)} fps` }))}
            onChange={(value) => {
              const [num, den] = value.split("/").map(Number);
              apply({ frameRate: { num: num!, den: den! } });
            }}
          />
        </Field>
        {hasClips && <Note>Frame rate is fixed once the timeline has clips.</Note>}
        <Field label="Sample rate">
          <Select
            value={String(format.sampleRate)}
            options={[
              { value: "44100", label: "44.1 kHz" },
              { value: "48000", label: "48 kHz" },
              { value: "96000", label: "96 kHz" },
            ]}
            onChange={(value) => apply({ sampleRate: Number(value) })}
          />
        </Field>
      </Section>
      <Note>Select a clip to see its details.</Note>
    </>
  );
}
