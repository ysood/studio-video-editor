// The assistant's panel. For now it only answers one question: who is signed in.
//
// That is deliberately the first thing built. Everything else the panel will do runs on
// a token, and the sign-in path is the only part that cannot be tested from a test run:
// it needs a real browser, a real account and a loopback port. So it gets a panel of its
// own first, where it can be exercised before anything depends on it.

import { useCallback, useEffect, useState } from "react";
import type { ChatStatus } from "../bindings/ChatStatus";
import { ipc, isChatError } from "../ipc";
import { isBrowserPreview } from "../preview/mode";
import { Button, EmptyState, Icon, Panel, PanelBody, PanelToolbar } from "../ui";
import "./chat.css";

/** What went wrong, and whether doing the same thing again is worth a try. */
type Trouble = { message: string; retryable: boolean };

function troubleOf(error: unknown): Trouble | null {
  if (isChatError(error)) {
    // A cancelled sign-in is somebody changing their mind. Reporting it back to them as
    // a failure would be telling them off for closing a tab.
    if (error.code === "cancelled") return null;
    return { message: error.message, retryable: error.retryable };
  }
  return {
    message: error instanceof Error ? error.message : String(error),
    retryable: true,
  };
}

export function ChatPanel() {
  const [status, setStatus] = useState<ChatStatus | null>(null);
  const [working, setWorking] = useState(false);
  const [trouble, setTrouble] = useState<Trouble | null>(null);

  useEffect(() => {
    let current = true;
    void ipc
      .chatStatus()
      .then((found) => current && setStatus(found))
      .catch(() => current && setStatus({ signedIn: false, email: null, canInfer: false }));
    return () => {
      current = false;
    };
  }, []);

  const run = useCallback(async (what: () => Promise<ChatStatus>) => {
    setWorking(true);
    setTrouble(null);
    try {
      setStatus(await what());
    } catch (error) {
      setTrouble(troubleOf(error));
    } finally {
      setWorking(false);
    }
  }, []);

  // Nothing is known yet. Shown blank rather than signed out, because guessing wrong for
  // one frame makes the panel flicker between two different layouts on every open.
  if (!status) return <Panel surface="stage" className="chat" />;

  return (
    <Panel surface="stage" className="chat">
      <PanelBody layout="sheet">
        {status.signedIn ? (
          <Account status={status} />
        ) : (
          <SignIn
            working={working}
            onSignIn={() => void run(ipc.chatSignIn)}
          />
        )}
        {trouble && (
          <p className="chat-trouble" role="alert">
            <Icon name="warning" />
            <span>{trouble.message}</span>
          </p>
        )}
      </PanelBody>
      {status.signedIn && (
        <PanelToolbar placement="bottom">
          <span className="chat-account">{status.email ?? "Signed in"}</span>
          <Button variant="ghost" disabled={working} onClick={() => void run(ipc.chatSignOut)}>
            Sign out
          </Button>
        </PanelToolbar>
      )}
    </Panel>
  );
}

function SignIn({ working, onSignIn }: { working: boolean; onSignIn: () => void }) {
  if (isBrowserPreview) {
    return (
      <EmptyState>
        Browser preview: signing in needs the desktop app.
      </EmptyState>
    );
  }
  return (
    <EmptyState
      action={
        <Button variant="solid" icon="chat" disabled={working} onClick={onSignIn}>
          {working ? "Waiting for your browser…" : "Sign in with ChatGPT"}
        </Button>
      }
    >
      Sign in to ask for edits in plain words. Studio uses your own ChatGPT plan, so
      there is no key to paste.
    </EmptyState>
  );
}

function Account({ status }: { status: ChatStatus }) {
  // Signed in and unable to ask anything is its own state, not a kind of signed out. It
  // happens when the plan was not shared during sign-in, and the fix is to sign in again
  // and allow it, which is nothing like signing in from scratch.
  if (!status.canInfer) {
    return (
      <EmptyState>
        Signed in, but ChatGPT plan access was not shared. Sign out and sign in again,
        allowing Studio to use your plan.
      </EmptyState>
    );
  }
  return (
    <EmptyState>
      Signed in and ready. Asking for edits comes next.
    </EmptyState>
  );
}
