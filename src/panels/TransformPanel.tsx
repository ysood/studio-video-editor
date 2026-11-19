import { useState } from "react";
import type { Clip } from "../bindings/Clip";
import type { Track } from "../bindings/Track";
import { setTransform, type TransformPatch } from "../edits";
import { useActiveSequence, useEditor } from "../store";
import { findClip } from "../timeline/geometry";
import { IDENTITY, MILLI, isIdentity, milli, units } from "../transform";
import {
  Actions,
  Button,
  Checkbox,
  EmptyState,
  Field,
  Note,
  NumberInput,
  Section,
  Slider,
} from "../ui";
import "./transform.css";

/** Percentages are what the UI shows; the document keeps plain factors in thousandths. */
const asPercent = (value: number) => Math.round(units(value) * 100);
const fromPercent = (value: number) => milli(value / 100);

/**
 * Where a clip's picture sits in the frame: Premiere's Motion and Opacity, in the panel
 * beside the Inspector.
 *
 * Only video clips have one. The audio half of a linked pair is left out rather than
 * shown disabled, because there is nothing about it to edit here.
 */
export function TransformBody() {
  const snapshot = useEditor((s) => s.snapshot);
  const selection = useEditor((s) => s.selection);
  const sequence = useActiveSequence();
  if (!snapshot?.project || !sequence) return null;

  const selected = selection
    .map((id) => findClip(sequence, id))
    .filter((f): f is { clip: Clip; track: Track } => f !== null);
  const video = selected.filter((e) => e.track.kind === "video");

  return (
    <>
      {selected.length === 0 && <EmptyState>Select a clip to move, scale or fade its picture.</EmptyState>}
      {selected.length > 0 && video.length === 0 && (
        <Section title="Transform">
          <Note>Sound has no picture to move.</Note>
        </Section>
      )}
      {video.length === 1 && <ClipTransform entry={video[0]!} />}
      {video.length > 1 && <ManyClips ids={video.map((e) => e.clip.id)} />}
    </>
  );
}

function ClipTransform({ entry }: { entry: { clip: Clip; track: Track } }) {
  const edit = useEditor((s) => s.edit);
  const { clip, track } = entry;
  const t = clip.transform;
  const locked = track.locked;
  // A tool setting rather than part of the document: Premiere keeps a Uniform Scale box
  // per clip, but nothing downstream cares, and one checkbox is one less thing to migrate.
  const [uniform, setUniform] = useState(true);

  const change = (patch: TransformPatch) => void edit(setTransform([clip.id], patch));
  const scaleTo = (x: number, y: number) => change({ scale: { x, y } });

  return (
    <>
      <Section title="Motion">
        <Field label="Position">
          <div className="transform-pair">
            <NumberInput
              value={units(t.position.x)}
              disabled={locked}
              ariaLabel="Position X"
              onCommit={(x) => change({ position: { x: milli(x), y: t.position.y } })}
            />
            <NumberInput
              value={units(t.position.y)}
              disabled={locked}
              ariaLabel="Position Y"
              onCommit={(y) => change({ position: { x: t.position.x, y: milli(y) } })}
            />
          </div>
        </Field>
        <Field label="Scale %">
          <div className="transform-pair">
            <NumberInput
              value={asPercent(t.scale.x)}
              disabled={locked}
              ariaLabel="Scale width"
              onCommit={(x) => scaleTo(fromPercent(x), uniform ? fromPercent(x) : t.scale.y)}
            />
            <NumberInput
              value={asPercent(t.scale.y)}
              disabled={locked || uniform}
              ariaLabel="Scale height"
              onCommit={(y) => scaleTo(t.scale.x, fromPercent(y))}
            />
          </div>
        </Field>
        <Field label="Uniform">
          <Checkbox
            checked={uniform}
            disabled={locked}
            ariaLabel="Scale width and height together"
            onChange={(on) => {
              setUniform(on);
              if (on && t.scale.y !== t.scale.x) scaleTo(t.scale.x, t.scale.x);
            }}
          />
        </Field>
        <Field label="Rotation °">
          <NumberInput
            value={units(t.rotation)}
            disabled={locked}
            ariaLabel="Rotation in degrees"
            onCommit={(degrees) => change({ rotation: milli(degrees) })}
          />
        </Field>
        <Field label="Anchor">
          <div className="transform-pair">
            <NumberInput
              value={units(t.anchor.x)}
              disabled={locked}
              ariaLabel="Anchor X"
              onCommit={(x) => change({ anchor: { x: milli(x), y: t.anchor.y } })}
            />
            <NumberInput
              value={units(t.anchor.y)}
              disabled={locked}
              ariaLabel="Anchor Y"
              onCommit={(y) => change({ anchor: { x: t.anchor.x, y: milli(y) } })}
            />
          </div>
        </Field>
      </Section>
      <Section title="Opacity">
        <Field label="Opacity %">
          <div className="transform-opacity">
            <Slider
              value={t.opacity}
              min={0}
              max={MILLI}
              step={10}
              ariaLabel="Opacity"
              onChange={(opacity) => change({ opacity })}
            />
            <NumberInput
              value={asPercent(t.opacity)}
              disabled={locked}
              ariaLabel="Opacity percentage"
              onCommit={(percent) => change({ opacity: fromPercent(percent) })}
            />
          </div>
        </Field>
      </Section>
      <Actions>
        <Button
          variant="ghost"
          disabled={locked || isIdentity(t)}
          onClick={() => void edit(setTransform([clip.id], { ...IDENTITY }))}
        >
          Reset transform
        </Button>
      </Actions>
    </>
  );
}

function ManyClips({ ids }: { ids: string[] }) {
  const edit = useEditor((s) => s.edit);
  return (
    <Section title={`${ids.length} clips selected`}>
      <Note>Pick one clip to move, scale or fade it.</Note>
      <Actions>
        <Button variant="ghost" onClick={() => void edit(setTransform(ids, { ...IDENTITY }))}>
          Reset every transform
        </Button>
      </Actions>
    </Section>
  );
}
