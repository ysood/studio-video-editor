// Asked once, when the first clip lands on an empty timeline and does not match the
// sequence. Conforming silently is how 25 fps footage loses a frame a second on a
// 24 fps timeline with nothing on screen to explain the judder, and the frame rate is
// fixed as soon as the timeline has clips, so there is no second chance to notice.

import type { SequenceFormat } from "../bindings/SequenceFormat";
import { placeMedia } from "../edits";
import { formatRate } from "../format";
import { useActiveSequence, useEditor } from "../store";
import { Actions, Button, Dialog, Note } from "../ui";

function describe(format: SequenceFormat): string {
  return `${format.width}×${format.height} at ${formatRate(format.frameRate)} fps`;
}

export function MatchSequenceDialog() {
  const pending = useEditor((s) => s.pendingInsert);
  const setPendingInsert = useEditor((s) => s.setPendingInsert);
  const edit = useEditor((s) => s.edit);
  const project = useEditor((s) => s.snapshot?.project);
  const sequence = useActiveSequence();
  const media = project?.media.find((m) => m.id === pending?.mediaId);

  const close = () => setPendingInsert(null);

  const place = async (adopt: boolean) => {
    if (!pending || !project || !sequence) return;
    // The format has to change before the clip arrives: once the timeline has a clip
    // the rate is locked, because every position in the document counts frames at it.
    if (adopt) {
      await edit({
        type: "SetSequenceFormat",
        sequenceId: sequence.id,
        format: pending.suggested,
      });
    }
    setPendingInsert(null);
    await placeMedia(pending.mediaId, pending.frame, pending.targets);
  };

  return (
    <Dialog open={pending !== null} onClose={close} title="This clip does not match the sequence">
      {pending && project && sequence && (
        <>
          <p>
            {media?.name ?? "The clip"} is {describe(pending.suggested)}, and the sequence is{" "}
            {describe(sequence.format)}.
          </p>
          <p>
            Matching the sequence to the clip keeps every source frame. Leaving it conforms the
            footage, which drops or repeats frames during playback and on export.
          </p>
          <Note>
            The frame rate is fixed once the timeline has clips, so it cannot be changed after this.
          </Note>
          <Actions align="end">
            <Button variant="ghost" onClick={() => void place(false)}>
              Keep sequence
            </Button>
            <Button variant="solid" onClick={() => void place(true)}>
              Match the clip
            </Button>
          </Actions>
        </>
      )}
    </Dialog>
  );
}
