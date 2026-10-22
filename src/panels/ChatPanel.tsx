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
import { useChat, type Entry } from "./chat-store";
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
  const entries = useChat((s) => s.entries);
  const streaming = useChat((s) => s.streaming);
  const send = useChat((s) => s.send);
  const append = useChat((s) => s.append);
  const noteTool = useChat((s) => s.noteTool);
  const [draft, setDraft] = useState("");
  const end = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const stops: Array<() => void> = [];
    let gone = false;
    const keep = (stop: () => void) => (gone ? stop() : stops.push(stop));
    void events.onChatDelta(({ text }) => append(text)).then(keep);
    void events.onChatTool((run) => noteTool(run)).then(keep);
    return () => {
      gone = true;
      stops.forEach((stop) => stop());
    };
  }, [append, noteTool]);

  // Follow the answer as it is written, the way a terminal does.
  useEffect(() => {
    end.current?.scrollIntoView({ block: "end" });
  }, [entries, streaming]);

  const asking = entries.some((entry) => entry.kind === "asking");
  const answering = streaming !== null || asking;
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
        placeholder={asking ? "Waiting on you" : answering ? "Answering…" : "Ask about your edit"}
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
      {entries.length === 0 && streaming === null ? (
        <EmptyState>
          Ask about what is open, or for an edit: trim a clip, move one, take one out.
        </EmptyState>
      ) : (
        <div className="chat-thread">
          {entries.map((entry, i) => (
            <Line key={i} entry={entry} />
          ))}
          {streaming !== null && <Said from="studio" text={streaming} pending />}
          <div ref={end} />
        </div>
      )}
    </Shell>
  );
}

function Line({ entry }: { entry: Entry }) {
  switch (entry.kind) {
    case "said":
      return <Said from={entry.from} text={entry.text} />;
    case "did":
      return (
        <p className="chat-did" data-ok={entry.ok}>
          <Icon name={entry.ok ? "check" : "warning"} />
          <span>{entry.detail ? `${entry.title}: ${entry.detail}` : entry.title}</span>
        </p>
      );
    case "answered":
      return (
        <p className="chat-did" data-ok={entry.approved}>
          <Icon name={entry.approved ? "check" : "close"} />
          <span>{entry.approved ? entry.title : `${entry.title}: not approved`}</span>
        </p>
      );
    case "note":
      return <p className="chat-did">{entry.text}</p>;
    case "asking":
      return <Asking entry={entry} />;
  }
}

/** One thing somebody said. */
function Said({ from, text, pending }: { from: "you" | "studio"; text: string; pending?: boolean }) {
  return (
    <div className="chat-said" data-from={from}>
      {/* An answer that has been asked for but not begun still needs to take up room,
          or the panel jumps when the first words arrive. */}
      {text === "" && pending ? <span className="chat-thinking" /> : text}
    </div>
  );
}

/**
 * What the assistant wants to do, and the two answers.
 *
 * The arguments are shown, not summarised. This is the one moment where what is about to
 * happen is still undone, and a paraphrase is exactly the thing that could be wrong.
 */
function Asking({ entry }: { entry: Extract<Entry, { kind: "asking" }> }) {
  const decide = useChat((s) => s.decide);
  return (
    <div className="chat-asking">
      <strong>{entry.title}</strong>
      <p>{entry.description}</p>
      <pre>{JSON.stringify(entry.arguments, null, 2)}</pre>
      <div className="chat-asking-actions">
        <Button variant="solid" onClick={() => void decide(true)}>
          Go ahead
        </Button>
        <Button variant="ghost" onClick={() => void decide(false)}>
          Don't
        </Button>
      </div>
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
