import { open, save } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import type { VideoCodec } from "../bindings/VideoCodec";
import { ipc } from "../ipc";
import { useEditor } from "../store";
import { Actions, Button, Dialog, RadioGroup, type RadioOption } from "../ui";
import "./export-dialog.css";
import { useActiveSequence } from "../store";

type Choice = "render-h264" | "render-prores" | "otio" | "fcp7" | "package";

const CHOICES: RadioOption[] = [
  { value: "render-h264", label: "Video file, H.264", detail: "MP4 for review and delivery." },
  { value: "render-prores", label: "Video file, ProRes 422 HQ", detail: "MOV intermediate for finishing." },
  { value: "package", label: "Conform package", detail: "A folder with every used file, OTIO, XML and a manifest. Best for handing to a colorist." },
  { value: "fcp7", label: "Timeline for Premiere or Resolve", detail: "FCP7 XML that points at your original files." },
  { value: "otio", label: "OpenTimelineIO", detail: "Interchange for Resolve and pipeline tools." },
];

/** Folder and file names cannot contain path separators or most punctuation. */
function safeName(name: string): string {
  return name.replace(/[/\\:*?"<>|]+/g, "-").trim() || "Untitled";
}

export function ExportDialog({ open: isOpen, onClose }: { open: boolean; onClose: () => void }) {
  const [choice, setChoice] = useState<Choice>("render-h264");
  const job = useEditor((s) => s.job);
  const sequence = useActiveSequence();
  const { notify, fail, setJob } = useEditor.getState();

  // Named after the sequence, not the project: every export here is of one sequence, and
  // a project with a rough cut beside a final would otherwise write both to one filename.
  // Premiere and Resolve both default to the timeline's name for the same reason.
  const name = safeName(sequence?.name ?? "Untitled");
  const sequenceId = sequence?.id ?? null;

  const run = async () => {
    if (!sequenceId) return;
    try {
      if (choice === "render-h264" || choice === "render-prores") {
        const codec: VideoCodec = choice === "render-h264" ? "h264" : "proRes422Hq";
        const ext = codec === "h264" ? "mp4" : "mov";
        const path = await save({ defaultPath: `${name}.${ext}`, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
        if (!path) return;
        await ipc.renderStart(sequenceId, path, { codec });
        setJob({ kind: "render", fraction: 0 });
        notify("info", `Rendering ${path}`);
      } else if (choice === "otio" || choice === "fcp7") {
        const ext = choice === "otio" ? "otio" : "xml";
        const path = await save({ defaultPath: `${name}.${ext}`, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
        if (!path) return;
        await ipc.exportInterchange(sequenceId, choice === "otio" ? "otio" : "fcp7Xml", path);
        notify("info", `Exported ${path}`);
      } else {
        const parent = await open({ directory: true, multiple: false, title: "Choose where to create the package" });
        if (typeof parent !== "string") return;
        const folder = `${parent.replace(/\/$/, "")}/${name}_Conform`;
        await ipc.exportPackage(sequenceId, folder);
        setJob({ kind: "package", fraction: 0 });
        notify("info", `Packaging into ${folder}`);
      }
      onClose();
    } catch (error) {
      fail(error);
    }
  };

  return (
    <Dialog open={isOpen} onClose={onClose} title="Export">
      <RadioGroup
        name="export"
        ariaLabel="Export type"
        value={choice}
        options={CHOICES}
        onChange={(value) => setChoice(value as Choice)}
      />
      {job && (
        <p className="export-busy">
          A {job.kind === "render" ? "render" : "package"} is running ({Math.round(job.fraction * 100)}%).{" "}
          {job.kind === "render" && (
            <Button onClick={() => void ipc.renderCancel()}>Cancel render</Button>
          )}
        </p>
      )}
      <Actions align="end">
        <Button variant="ghost" onClick={onClose}>Close</Button>
        <Button variant="solid" onClick={() => void run()} disabled={job !== null}>
          Export
        </Button>
      </Actions>
    </Dialog>
  );
}
