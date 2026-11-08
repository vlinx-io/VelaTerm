import { useCallback, useEffect, useRef, useState } from "react";
import { chatAgentReadiness, type AgentReadiness } from "../../../ipc/chat";

/** Coming back to the view checks again, so an install or sign-in done elsewhere counts, but not on every return. */
const RECHECK_MS = 15_000;

/**
 * Whether this conversation's agent is installed and signed in, read when the conversation opens and again
 * when its account state changes. `null` until answered, and after a failed check: neither holds a message back.
 */
export function useAgentReadiness(sessionId: string, enabled: boolean, authStatus: string | undefined) {
  const [readiness, setReadiness] = useState<AgentReadiness | null>(null);
  const latest = useRef(0);
  const checkedAt = useRef(0);

  const refresh = useCallback(() => {
    const request = ++latest.current;
    checkedAt.current = Date.now();
    chatAgentReadiness(sessionId).then(
      value => { if (request === latest.current) setReadiness(value ?? null); },
      () => { if (request === latest.current) setReadiness(null); },
    );
  }, [sessionId]);

  useEffect(() => {
    latest.current += 1;
    checkedAt.current = 0;
    setReadiness(null);
  }, [sessionId]);

  // Each check starts the agent's own CLI, so moving between tabs reuses a recent answer.
  useEffect(() => {
    if (!enabled) return;
    const recheck = () => {
      if (Date.now() - checkedAt.current >= RECHECK_MS) refresh();
    };
    recheck();
    window.addEventListener("focus", recheck);
    return () => window.removeEventListener("focus", recheck);
  }, [enabled, refresh]);

  // A finished sign-in or sign-out changes the answer. Steps within a sign-in do not.
  const settled = authStatus === "success" || authStatus === "signedOut" || authStatus === undefined ? authStatus ?? "none" : null;
  const lastSettled = useRef(settled);
  useEffect(() => {
    if (settled === null || settled === lastSettled.current) return;
    lastSettled.current = settled;
    if (enabled) refresh();
  }, [settled, enabled, refresh]);

  return { readiness, refresh };
}
