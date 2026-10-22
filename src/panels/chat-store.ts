// The conversation as the panel shows it.
//
// Rust holds the real one: what was said, in the order it happened, in the shape the
// model is sent. This holds what to draw, which is not the same list. A turn the model
// spent reasoning is an item there and nothing here; a tool it ran is one line here and
// two items there; a question that failed is nothing there and a line to read here.

import { create } from "zustand";
import type { ChatSettings } from "../bindings/ChatSettings";
import type { ChatStatus } from "../bindings/ChatStatus";
import type { ChatToolRun } from "../bindings/ChatToolRun";
import type { TurnEnd } from "../bindings/TurnEnd";
import { ipc, isChatError } from "../ipc";

/** One line of the transcript. */
export type Entry =
  | { kind: "said"; from: "you" | "studio"; text: string }
  /** Something the assistant did to the document. */
  | { kind: "did"; tool: string; title: string; ok: boolean; detail: string | null }
  /** Something it wants to do, waiting to be told whether to. */
  | { kind: "asking"; tool: string; title: string; description: string; arguments: unknown }
  /** How that question was answered, kept so the transcript still reads in order. */
  | { kind: "answered"; title: string; approved: boolean }
  /** Something the panel itself has to say. Never dressed up as the assistant. */
  | { kind: "note"; text: string };

export interface Trouble {
  message: string;
  retryable: boolean;
}

function troubleOf(error: unknown): Trouble | null {
  if (isChatError(error)) {
    // Somebody closing the sign-in tab has changed their mind. Reporting that back to
    // them as a failure would be telling them off for it.
    if (error.code === "cancelled") return null;
    return { message: error.message, retryable: error.retryable };
  }
  return {
    message: error instanceof Error ? error.message : String(error),
    retryable: true,
  };
}

interface ChatState {
  /** Null until the first answer about who is signed in. */
  status: ChatStatus | null;
  /** Null until read. Both of its fields may be null, which means "no opinion". */
  settings: ChatSettings | null;
  /** Whether the settings dialog is open. */
  settingsOpen: boolean;
  entries: Entry[];
  /** The answer so far, while it is being written. Null when nothing is in flight. */
  streaming: string | null;
  /** True while a sign-in or sign-out is in progress. */
  busy: boolean;
  trouble: Trouble | null;

  load: () => Promise<void>;
  choose: (model: string | null, effort: string | null) => Promise<void>;
  setSettingsOpen: (open: boolean) => void;
  signIn: () => Promise<void>;
  signOut: () => Promise<void>;
  send: (text: string) => Promise<void>;
  decide: (approved: boolean) => Promise<void>;
  reset: () => Promise<void>;
  /** Called as `chat://delta` arrives. */
  append: (text: string) => void;
  /** Called as `chat://tool` arrives. */
  noteTool: (run: ChatToolRun) => void;
}

export const useChat = create<ChatState>((set, get) => ({
  status: null,
  settings: null,
  settingsOpen: false,
  entries: [],
  streaming: null,
  busy: false,
  trouble: null,

  load: async () => {
    try {
      set({ status: await ipc.chatStatus() });
    } catch {
      // Outside the desktop app there is nothing to be signed in to.
      set({ status: { signedIn: false, email: null, canInfer: false } });
    }
    try {
      set({ settings: await ipc.chatSettings() });
    } catch {
      set({ settings: { model: null, effort: null } });
    }
  },

  choose: async (model, effort) => {
    try {
      const settings = await ipc.chatSetSettings(model, effort);
      // Changing the model starts a new conversation in Rust, so the transcript here
      // has to go with it. Leaving it on screen would show a thread the model no
      // longer has, and the next answer would read as a non sequitur.
      const restart = settings.model !== get().settings?.model;
      set((state) => ({
        settings,
        entries: restart ? [] : state.entries,
        streaming: restart ? null : state.streaming,
      }));
    } catch (error) {
      set({ trouble: troubleOf(error) });
    }
  },

  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),

  signIn: async () => {
    set({ busy: true, trouble: null });
    try {
      set({ status: await ipc.chatSignIn() });
    } catch (error) {
      set({ trouble: troubleOf(error) });
    } finally {
      set({ busy: false });
    }
  },

  signOut: async () => {
    set({ busy: true, trouble: null });
    try {
      const status = await ipc.chatSignOut();
      // The conversation goes with the account. Leaving it on screen under a sign-in
      // prompt would invite whoever signs in next to read it.
      await ipc.chatReset().catch(() => undefined);
      set({ status, entries: [], streaming: null });
    } catch (error) {
      set({ trouble: troubleOf(error) });
    } finally {
      set({ busy: false });
    }
  },

  send: async (text) => {
    const message = text.trim();
    if (message === "" || get().streaming !== null || get().entries.some(isAsking)) return;
    set((state) => ({
      entries: [...state.entries, { kind: "said", from: "you", text: message }],
      // Empty rather than null: the panel reads this to know an answer is coming, and
      // shows the waiting state until the first piece of it arrives.
      streaming: "",
      trouble: null,
    }));
    await settle(set, () => ipc.chatSend(message));
  },

  decide: async (approved) => {
    const asking = get().entries.find(isAsking);
    if (!asking) return;
    set((state) => ({
      // The question becomes the record of how it was answered, in the same place, so
      // the transcript still reads in the order things happened.
      entries: state.entries.map((entry) =>
        entry === asking
          ? { kind: "answered", title: asking.title, approved }
          : entry,
      ),
      streaming: "",
      trouble: null,
    }));
    await settle(set, () => ipc.chatApprove(approved));
  },

  reset: async () => {
    await ipc.chatReset().catch(() => undefined);
    set({ entries: [], streaming: null, trouble: null });
  },

  append: (text) =>
    set((state) => (state.streaming === null ? state : { streaming: state.streaming + text })),

  noteTool: (run) =>
    set((state) => ({
      entries: [
        ...state.entries,
        { kind: "did", tool: run.tool, title: run.title, ok: run.ok, detail: run.detail },
      ],
    })),
}));

function isAsking(entry: Entry): entry is Extract<Entry, { kind: "asking" }> {
  return entry.kind === "asking";
}

/** Wait for a turn to end, and put whatever it ended as into the transcript. */
async function settle(
  set: (fn: (state: ChatState) => Partial<ChatState>) => void,
  run: () => Promise<TurnEnd>,
) {
  try {
    const end = await run();
    set((state) => ({
      streaming: null,
      entries:
        end.kind === "answered"
          ? // The whole answer replaces what was streamed rather than being added to
            // it, so a piece that arrived twice, or not at all, does not survive. A
            // turn that said nothing at all is reported rather than dropped: silence
            // looks exactly like a panel that is broken, which is how this went unseen.
            [
              ...state.entries,
              end.text === ""
                ? { kind: "note", text: "The assistant finished without saying anything." }
                : { kind: "said", from: "studio", text: end.text },
            ]
          : [
              ...state.entries,
              {
                kind: "asking",
                tool: end.tool,
                title: end.title,
                description: end.description,
                arguments: end.arguments,
              },
            ],
    }));
  } catch (error) {
    set(() => ({ streaming: null, trouble: troubleOf(error) }));
  }
}
