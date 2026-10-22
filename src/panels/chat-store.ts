// The conversation as the panel sees it.
//
// Rust holds the real one: what was said, in the order it happened, in the shape the
// model is sent. This holds what to draw, which is not the same list. A turn the model
// spent on reasoning is one item there and nothing here; a question that failed is
// nothing there and a line to read here.

import { create } from "zustand";
import type { ChatStatus } from "../bindings/ChatStatus";
import { ipc, isChatError } from "../ipc";

export interface Message {
  from: "you" | "studio";
  text: string;
}

/** A failure worth showing, and whether doing the same thing again could work. */
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
  messages: Message[];
  /** The answer so far, while it is being written. Null when nothing is in flight. */
  streaming: string | null;
  /** True while a sign-in or sign-out is in progress. */
  busy: boolean;
  trouble: Trouble | null;

  load: () => Promise<void>;
  signIn: () => Promise<void>;
  signOut: () => Promise<void>;
  send: (text: string) => Promise<void>;
  reset: () => Promise<void>;
  /** Called as `chat://delta` arrives. */
  append: (text: string) => void;
}

export const useChat = create<ChatState>((set, get) => ({
  status: null,
  messages: [],
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
  },

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
      // prompt would invite somebody else to read it.
      await ipc.chatReset().catch(() => undefined);
      set({ status, messages: [], streaming: null });
    } catch (error) {
      set({ trouble: troubleOf(error) });
    } finally {
      set({ busy: false });
    }
  },

  send: async (text) => {
    const message = text.trim();
    if (message === "" || get().streaming !== null) return;
    set((state) => ({
      messages: [...state.messages, { from: "you", text: message }],
      // Empty rather than null: the panel reads this to know an answer is coming, and
      // shows the waiting state until the first piece of it arrives.
      streaming: "",
      trouble: null,
    }));
    try {
      const answer = await ipc.chatSend(message);
      // The whole answer replaces what was streamed rather than being added to it, so a
      // piece that arrived twice or not at all does not survive into the transcript.
      set((state) => ({
        messages: [...state.messages, { from: "studio", text: answer }],
        streaming: null,
      }));
    } catch (error) {
      set({ streaming: null, trouble: troubleOf(error) });
    }
  },

  reset: async () => {
    await ipc.chatReset().catch(() => undefined);
    set({ messages: [], streaming: null, trouble: null });
  },

  append: (text) =>
    set((state) => (state.streaming === null ? state : { streaming: state.streaming + text })),
}));
