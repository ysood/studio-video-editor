// Naming a project and choosing where it goes, before it exists.
//
// Premiere asks for both up front and writes the file on Create; Resolve names the
// project into a library at the same moment. Studio follows them, because a project
// that exists only in memory until someone remembers to save is a project with no
// entry in the recents list, nowhere for an auto-save to go, and no answer to "where
// did it end up".

import { open } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";

import { DEFAULT_FORMAT } from "../actions";
import { ipc } from "../ipc";
import { useEditor } from "../store";
import { Actions, Button, Dialog, Field, Note, TextInput } from "../ui";
import "./home.css";

/** Folder and file names cannot contain path separators or most punctuation. */
function safeName(name: string): string {
  return name.replace(/[/\\:*?"<>|]+/g, "-").trim();
}

export function NewProjectDialog({ open: isOpen, onClose }: { open: boolean; onClose: () => void }) {
  const [name, setName] = useState("Untitled");
  const [folder, setFolder] = useState<string | null>(null);
  const fail = useEditor((s) => s.fail);
  const setSnapshot = useEditor((s) => s.setSnapshot);

  // A fresh name each time the dialog opens, so yesterday's typing is not in the way.
  useEffect(() => {
    if (isOpen) setName("Untitled");
  }, [isOpen]);

  const file = safeName(name) || "Untitled";
  const ready = folder !== null && safeName(name).length > 0;

  const choose = async () => {
    try {
      const picked = await open({ directory: true, multiple: false, title: "Where should the project live?" });
      if (typeof picked === "string") setFolder(picked);
    } catch (error) {
      fail(error);
    }
  };

  const create = async () => {
    if (!ready) return;
    try {
      const path = `${folder.replace(/\/$/, "")}/${file}.studio`;
      // Make it, then write it, so the project has a home from its first moment.
      await ipc.newProject(file, DEFAULT_FORMAT);
      setSnapshot(await ipc.saveProject(path));
      onClose();
    } catch (error) {
      fail(error);
    }
  };

  return (
    <Dialog open={isOpen} onClose={onClose} title="New project">
      <Field label="Name">
        <TextInput value={name} onCommit={setName} ariaLabel="Project name" />
      </Field>
      <Field label="Location">
        <Button variant="quiet" onClick={() => void choose()}>
          {folder ?? "Choose a folder…"}
        </Button>
      </Field>
      <Note>
        {folder ? `Saved as ${file}.studio in that folder.` : "The project file is written when you create it."}
      </Note>
      <Actions align="end">
        <Button variant="ghost" onClick={onClose}>
          Cancel
        </Button>
        <Button variant="solid" disabled={!ready} onClick={() => void create()}>
          Create
        </Button>
      </Actions>
    </Dialog>
  );
}
