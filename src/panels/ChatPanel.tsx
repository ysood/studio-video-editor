// The assistant's panel: a conversation, and what it takes to have one.
//
// Three states, and they are genuinely different rather than variations on a theme.
// Signed out, there is nothing to say and one thing to do. Signed in without the plan
// shared, the sign-in worked and still nothing can be asked, which needs its own words.
// Signed in properly, it is a chat.

import { useEffect, useRef, useState, type ReactNode } from "react";
import { events } from "../ipc";
import { isBrowserPreview } from "../preview/mode";
import { Button, EmptyState, Icon, IconButton, Panel, PanelBody, PanelToolbar } from "../ui";
import { useChat, type Message } from "./chat-store";
import "./chat.css";

export function ChatPanel() {
  const status = useChat((s) => s.status);
  const load = useChat((s) => s.load);

  useEffect(() => {
    void load();
  }, [load]);

  // Nothing known yet. Blank rather than signed out: guessing wrong for one frame makes
  // the panel flick between two different layouts every time it opens.
  if (!status) return <Panel surface="stage" className="chat" />;
  if (!status.signedIn) return <SignedOut />;
  if (!status.canInfer) return <PlanNotShared />;
  return <Conversation />;
}

function SignedOut() {
  const busy = useChat((s) => s.busy);
  const signIn = useChat((s) => s.signIn);
  return (
    <Shell>
      {isBrowserPreview ? (
        <EmptyState>Browser preview: signing in needs the desktop app.</EmptyState>
      ) : (
        <EmptyState
          action={
            <Button variant="solid" icon="chat" disabled={busy} onClick={() => void signIn()}>
              {busy ? "Waiting for your browser…" : "Sign in with ChatGPT"}
            </Button>
          }
        >
          Sign in to ask about your edit in plain words. Studio uses your own ChatGPT
          plan, so there is no key to paste.
        </EmptyState>
      )}
    </Shell>
  );
}

function PlanNotShared() {
  const busy = useChat((s) => s.busy);
  const signOut = useChat((s) => s.signOut);
  return (
    <Shell>
      <EmptyState
        action={
          <Button variant="ghost" disabled={busy} onClick={() => void signOut()}>
            Sign out and start again
          </Button>
        }
      >
        Signed in, but Studio was not given permission to use your ChatGPT plan. Sign in
        again and allow it.
      </EmptyState>
    </Shell>
  );
}

function Conversation() {
  const messages = useChat((s) => s.messages);
  const streaming = useChat((s) => s.streaming);
  const send = useChat((s) => s.send);
  const append = useChat((s) => s.append);
  const [draft, setDraft] = useState("");
  const end = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void events.onChatDelta(({ text }) => append(text)).then((stop) => {
      unlisten = stop;
    });
    return () => unlisten?.();
  }, [append]);

  // Follow the answer as it is written, the way a terminal does.
  useEffect(() => {
    end.current?.scrollIntoView({ block: "end" });
  }, [messages, streaming]);

  const answering = streaming !== null;
  const submit = () => {
    if (draft.trim() === "" || answering) return;
    void send(draft);
    setDraft("");
  };

  const composer = (
    <PanelToolbar placement="bottom">
      <textarea
        className="chat-draft"
        rows={1}
        value={draft}
        placeholder={answering ? "Answering…" : "Ask about your edit"}
        aria-label="Ask the assistant"
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          // Enter sends, since that is what every other chat does. A newline is still
          // reachable, which matters for anyone pasting something with shape to it.
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            submit();
          }
        }}
      />
      <IconButton
        icon="send"
        label="Send"
        disabled={draft.trim() === "" || answering}
        onClick={submit}
      />
    </PanelToolbar>
  );

  return (
    <Shell footer={composer}>
      {messages.length === 0 && streaming === null ? (
        <EmptyState>
          Ask about what is open: what is on the timeline, how long a sequence runs,
          where a clip sits.
        </EmptyState>
      ) : (
        <div className="chat-thread">
          {messages.map((message, i) => (
            <Said key={i} message={message} />
          ))}
          {streaming !== null && <Said message={{ from: "studio", text: streaming }} pending />}
          <div ref={end} />
        </div>
      )}
    </Shell>
  );
}

/** One thing somebody said. */
function Said({ message, pending }: { message: Message; pending?: boolean }) {
  return (
    <div className="chat-said" data-from={message.from}>
      {/* An answer that has been asked for but not begun still needs to take up room,
          or the panel jumps when the first words arrive. */}
      {message.text === "" && pending ? <span className="chat-thinking" /> : message.text}
    </div>
  );
}

/** The panel, with whatever trouble there is shown between its content and its footer. */
function Shell({ children, footer }: { children: ReactNode; footer?: ReactNode }) {
  const trouble = useChat((s) => s.trouble);
  return (
    <Panel surface="stage" className="chat">
      <PanelBody>{children}</PanelBody>
      {trouble && (
        <p className="chat-trouble" role="alert">
          <Icon name="warning" />
          <span>{trouble.message}</span>
        </p>
      )}
      {footer}
    </Panel>
  );
}

/**
 * The panel's own entries in the dock header's menu. They belong there rather than in a
 * row of their own: both are rare, and a chat panel's space is for the chat.
 */
export function chatMenu() {
  const { reset, signOut, status } = useChat.getState();
  return [
    { label: "New conversation", icon: "plus" as const, onSelect: () => void reset() },
    {
      label: "Sign out of ChatGPT",
      disabled: !status?.signedIn,
      onSelect: () => void signOut(),
    },
  ];
}
