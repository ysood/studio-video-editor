// Which model answers, and how hard it is allowed to think.
//
// Both choices can be left unmade, and that is the default. A slug outlives the model it
// names, so "whatever the service lists first" has to stay sayable: it is the only
// setting that keeps working when the catalogue moves on.

import { useEffect, useState } from "react";
import type { ChatModel } from "../bindings/ChatModel";
import { errorMessage, ipc } from "../ipc";
import { Actions, Button, Dialog, Field, Note, Select } from "../ui";
import { useChat } from "./chat-store";

/**
 * What the stored form calls "no opinion", spelled for a select.
 *
 * Not the empty string, which is how a select says "nothing is chosen at all": using it
 * here leaves the control blank rather than showing that automatic is the choice. A
 * model slug is lower case letters, digits, dots and dashes, so this cannot be mistaken
 * for one.
 */
const AUTO = "auto!";

export function ChatSettingsDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const settings = useChat((s) => s.settings);
  const choose = useChat((s) => s.choose);
  const hasConversation = useChat((s) => s.entries.length > 0);

  const [models, setModels] = useState<ChatModel[] | null>(null);
  const [trouble, setTrouble] = useState<string | null>(null);
  const [model, setModel] = useState(AUTO);
  const [effort, setEffort] = useState(AUTO);

  // Loaded on open rather than held: the catalogue is a network call, and it changes
  // rarely enough that asking at the moment somebody wants to see it is right.
  useEffect(() => {
    if (!open) return;
    let current = true;
    setTrouble(null);
    setModel(settings?.model ?? AUTO);
    setEffort(settings?.effort ?? AUTO);
    void ipc
      .chatModels()
      .then((found) => current && setModels(found))
      .catch((error) => current && setTrouble(errorMessage(error)));
    return () => {
      current = false;
    };
  }, [open, settings]);

  const chosen = models?.find((m) => m.slug === model);
  // Without a chosen model there is no list of levels to offer: they are not the same
  // for every model, and offering one a model refuses would fail at the next question.
  const efforts = chosen?.efforts ?? [];
  const automatic = models?.[0];

  const save = async () => {
    await choose(model === AUTO ? null : model, effort === AUTO ? null : effort);
    onClose();
  };

  return (
    <Dialog open={open} onClose={onClose} title="Assistant">
      {trouble ? (
        <Note>{trouble}</Note>
      ) : (
        <>
          <Field label="Model">
            <Select
              value={model}
              ariaLabel="Model"
              onChange={(next) => {
                setModel(next);
                // The levels belong to the model, so a level chosen for the old one is
                // not carried over to a model that may not have it.
                setEffort(AUTO);
              }}
              options={[
                {
                  value: AUTO,
                  label: automatic ? `Automatic (${automatic.name})` : "Automatic",
                },
                ...(models ?? []).map((m) => ({ value: m.slug, label: m.name })),
              ]}
            />
          </Field>
          {chosen?.description && <Note>{chosen.description}</Note>}

          <Field label="Thinking">
            <Select
              value={effort}
              ariaLabel="Thinking"
              disabled={efforts.length === 0}
              onChange={setEffort}
              options={[
                {
                  value: AUTO,
                  label: chosen?.defaultEffort
                    ? `Automatic (${chosen.defaultEffort})`
                    : "Automatic",
                },
                ...efforts.map((e) => ({ value: e.effort, label: e.effort })),
              ]}
            />
          </Field>
          {efforts.length === 0 ? (
            <Note>Choose a model to set how hard it thinks.</Note>
          ) : (
            <Note>{efforts.find((e) => e.effort === effort)?.description ?? ""}</Note>
          )}

          {hasConversation && model !== (settings?.model ?? AUTO) && (
            <Note>
              Changing the model starts a new conversation: what a model produced belongs
              to the model that made it.
            </Note>
          )}
        </>
      )}
      <Actions align="end">
        <Button variant="ghost" onClick={onClose}>
          Cancel
        </Button>
        <Button variant="solid" disabled={!models} onClick={() => void save()}>
          Done
        </Button>
      </Actions>
    </Dialog>
  );
}
