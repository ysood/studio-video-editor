import { open, save } from "@tauri-apps/plugin-dialog";
import { useEffect, useRef, useState } from "react";
import type { VideoCodec } from "../bindings/VideoCodec";
import { ipc } from "../ipc";
import { useEditor } from "../store";
import "./export-dialog.css";

type Choice = "render-h264" | "render-prores" | "otio" | "fcp7" | "package";

const CHOICES: { id: Choice; title: string; detail: string }[] = [
  { id: "render-h264", title: "Video file, H.264", detail: "MP4 for review and delivery." },
  { id: "render-prores", title: "Video file, ProRes 422 HQ", detail: "MOV intermediate for finishing." },
  { id: "package", title: "Conform package", detail: "A folder with every used file, OTIO, XML and a manifest. Best for handing to a colorist." },
  { id: "fcp7", title: "Timeline for Premiere or Resolve", detail: "FCP7 XML that points at your original files." },
  { id: "otio", title: "OpenTimelineIO", detail: "Interchange for Resolve and pipeline tools." },
];

/** Folder and file names cannot contain path separators or most punctuation. */
function safeName(name: string): string {
  return name.replace(/[/\\:*?"<>|]+/g, "-").trim() || "Untitled";
}

export function ExportDialog({ open: isOpen, onClose }: { open: boolean; onClose: () => void }) {
  const ref = useRef<HTMLDialogElement>(null);
  const [choice, setChoice] = useState<Choice>("render-h264");
  const snapshot = useEditor((s) => s.snapshot);
  const job = useEditor((s) => s.job);
  const { notify, fail, setJob } = useEditor.getState();

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (isOpen && !dialog.open) dialog.showModal();
    if (!isOpen && dialog.open) dialog.close();
  }, [isOpen]);

  const name = safeName(snapshot?.project.name ?? "Untitled");

  const run = async () => {
    try {
      if (choice === "render-h264" || choice === "render-prores") {
        const codec: VideoCodec = choice === "render-h264" ? "h264" : "proRes422Hq";
        const ext = codec === "h264" ? "mp4" : "mov";
        const path = await save({ defaultPath: `${name}.${ext}`, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
        if (!path) return;
        await ipc.renderStart(path, { codec });
        setJob({ kind: "render", fraction: 0 });
        notify("info", `Rendering ${path}`);
      } else if (choice === "otio" || choice === "fcp7") {
        const ext = choice === "otio" ? "otio" : "xml";
        const path = await save({ defaultPath: `${name}.${ext}`, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
        if (!path) return;
        await ipc.exportInterchange(choice === "otio" ? "otio" : "fcp7Xml", path);
        notify("info", `Exported ${path}`);
      } else {
        const parent = await open({ directory: true, multiple: false, title: "Choose where to create the package" });
        if (typeof parent !== "string") return;
        const folder = `${parent.replace(/\/$/, "")}/${name}_Conform`;
        await ipc.exportPackage(folder);
        setJob({ kind: "package", fraction: 0 });
        notify("info", `Packaging into ${folder}`);
      }
      onClose();
    } catch (error) {
      fail(error);
    }
  };

  return (
    <dialog ref={ref} className="export-dialog" onClose={onClose} onCancel={onClose}>
      <h2>Export</h2>
      <div className="export-choices" role="radiogroup" aria-label="Export type">
        {CHOICES.map((c) => (
          <label key={c.id} className="export-choice" data-checked={choice === c.id}>
            <input
              type="radio"
              name="export"
              value={c.id}
              checked={choice === c.id}
              onChange={() => setChoice(c.id)}
            />
            <span>
              <strong>{c.title}</strong>
              <small>{c.detail}</small>
            </span>
          </label>
        ))}
      </div>
      {job && (
        <p className="export-busy">
          A {job.kind === "render" ? "render" : "package"} is running ({Math.round(job.fraction * 100)}%).{" "}
          {job.kind === "render" && (
            <button className="btn" onClick={() => void ipc.renderCancel()}>Cancel render</button>
          )}
        </p>
      )}
      <div className="export-actions">
        <button className="btn" onClick={onClose}>Close</button>
        <button className="btn btn-primary" onClick={() => void run()} disabled={job !== null}>
          Export
        </button>
      </div>
    </dialog>
  );
}
