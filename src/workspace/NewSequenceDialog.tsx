// Creating a timeline, reached from the + beside Timelines in the bin.
//
// The format starts as a copy of the timeline you are on, which is Resolve's "use
// project settings" without a second concept: in practice the next sequence almost
// always matches the last, and the fields are right there when it does not.

import { useEffect, useState } from "react";

import type { SequenceFormat } from "../bindings/SequenceFormat";
import { formatRate } from "../format";
import { FRAME_RATES, SIZES } from "../panels/InspectorPanel";
import { useActiveSequence, useEditor } from "../store";
import { Actions, Button, Dialog, Field, Select, TextInput } from "../ui";

export function NewSequenceDialog({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const edit = useEditor((s) => s.edit);
  const setActiveSequence = useEditor((s) => s.setActiveSequence);
  // Selecting a count, not a derived array: a selector that builds a new array every
  // call never compares equal and spins the store forever.
  const sequenceCount = useEditor(
    (s) => s.snapshot?.project.sequences.length ?? 0,
  );
  const current = useActiveSequence();
  const [name, setName] = useState("");
  const [format, setFormat] = useState<SequenceFormat | null>(null);

  // Reopening starts from the timeline on screen rather than from whatever was typed
  // the last time the dialog was open.
  useEffect(() => {
    if (!open || !current) return;
    setName(`Sequence ${sequenceCount + 1}`);
    setFormat(current.format);
  }, [open, current, sequenceCount]);

  const create = async () => {
    if (!format) return;
    const report = await edit({
      type: "AddSequence",
      name: name.trim() || "Sequence",
      format,
    });
    if (report?.createdSequence) setActiveSequence(report.createdSequence);
    onClose();
  };

  // One Dialog element either way. Returning a different one while it is mounted
  // detaches and reattaches its ref, which loops.
  const sizeKey = format ? `${format.width}x${format.height}` : "";
  return (
    <Dialog open={open} onClose={onClose} title="New timeline">
      {format && (
        <>
          <Field label="Name">
            <TextInput
              value={name}
              onCommit={setName}
              ariaLabel="Timeline name"
            />
          </Field>
          <Field label="Frame size">
            <Select
              value={sizeKey}
              options={SIZES.map((s) => ({
                value: `${s.width}x${s.height}`,
                label: s.label,
              }))}
              onChange={(value) => {
                const size = SIZES.find(
                  (s) => `${s.width}x${s.height}` === value,
                );
                if (size)
                  setFormat({
                    ...format,
                    width: size.width,
                    height: size.height,
                  });
              }}
              ariaLabel="Frame size"
            />
          </Field>
          <Field label="Frame rate">
            <Select
              value={`${format.frameRate.num}/${format.frameRate.den}`}
              options={FRAME_RATES.map((r) => ({
                value: `${r.num}/${r.den}`,
                label: `${formatRate(r)} fps`,
              }))}
              onChange={(value) => {
                const [num, den] = value.split("/").map(Number);
                setFormat({ ...format, frameRate: { num: num!, den: den! } });
              }}
              ariaLabel="Frame rate"
            />
          </Field>
          <Actions align="end">
            <Button variant="ghost" onClick={onClose}>
              Cancel
            </Button>
            <Button variant="solid" onClick={() => void create()}>
              Create
            </Button>
          </Actions>
        </>
      )}
    </Dialog>
  );
}
