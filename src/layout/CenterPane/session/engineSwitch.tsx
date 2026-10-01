//! Moving a session between its two views, and the one question that move sometimes has to ask.
//!
//! The terminal view and the conversation view are not two readings of one process: the terminal runs the
//! agent's own interface, the conversation runs it as a protocol peer. Switching therefore stops one and
//! starts the other. The conversation survives — both engines write the same record and pick it back up —
//! but a turn in flight does not, so a working agent is asked about first and an idle one is simply moved.

import { useEffect, useRef, useState } from "react";

import { Backdrop } from "../../../components/Backdrop";
import { useT } from "../../../i18n";
import { useTermStore } from "../../../store/termStore";
import { sanitizePrefill } from "../../../terminal/prefill";
import { effectiveStatus, type Session, type SessionEngine } from "../../../types";

export interface SwitchOptions {
  /** Typed into the terminal view's agent prompt, never submitted. */
  prefill?: string;
  /** Called once the switch really begins, after any question was answered with yes. */
  onStart?: () => void;
  /** Called once the server has switched, also when only a fresh read of the tree shows that it did. */
  onDone?: () => void;
  /** Called when a begun switch fails or its outcome cannot be read. */
  onFail?: () => void;
}

/** The engine the server has for a session, read fresh; undefined when the tree cannot be read. */
async function engineOnServer(id: string): Promise<SessionEngine | undefined> {
  try {
    await useTermStore.getState().loadTree();
  } catch {
    return undefined;
  }
  const found = useTermStore.getState().sessions.find((s) => s.id === id);
  return found ? (found.engine ?? "tui") : undefined;
}

/** Drops a prefill this switch left, unless a newer one has replaced it or the terminal took it. */
function clearPrefill(id: string, text: string) {
  const store = useTermStore.getState();
  if (store.runtimes[id]?.agentPrefill === text) store.setRuntime(id, { agentPrefill: undefined });
}

export function useEngineSwitch(session: Session) {
  const t = useT();
  const status = useTermStore((s) => effectiveStatus(s.runtimes[session.id]));
  // The engine waiting on an answer, or null when nothing is being asked.
  const [asking, setAsking] = useState<SessionEngine | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const switching = useRef(false);
  // What the caller asked to happen with the switch; held while the question is open, dropped on cancel.
  const pending = useRef<SwitchOptions>({});
  // A prefill left in place after a failure whose outcome could not be read, with the tree it was left under.
  const unresolved = useRef<{ text: string; sessions: Session[] } | null>(null);

  const cancel = () => {
    pending.current = {};
    setAsking(null);
  };

  const apply = (engine: SessionEngine) => {
    if (switching.current) return;
    switching.current = true;
    setAsking(null);
    setFailure(null);
    const store = useTermStore.getState();
    const { prefill, onStart, onDone, onFail } = pending.current;
    pending.current = {};
    const text = engine === "tui" && prefill ? sanitizePrefill(prefill) : "";
    // Set before the switch, so the terminal finds it when it spawns; cleared again if the switch fails.
    if (text) store.setRuntime(session.id, { agentPrefill: text });
    onStart?.();
    void store
      .setSessionEngineMode(session.id, engine)
      .then(() => { onDone?.(); }, async (error) => {
        // A failed request does not prove a failed switch: a connection that drops after the server
        // committed loses the answer, not the switch. Only a fresh read of the tree decides what to undo.
        const now = await engineOnServer(session.id);
        if (now === engine) {
          onDone?.();
          return;
        }
        if (now === undefined) {
          // Unknown: keep the prefill for a terminal that may still open, and give the draft back for a
          // conversation that may stay. The next tree read settles which one survives.
          if (text) unresolved.current = { text, sessions: useTermStore.getState().sessions };
        } else if (text) {
          clearPrefill(session.id, text);
        }
        onFail?.();
        setFailure(String(error));
      })
      .finally(() => { switching.current = false; });
  };

  // Settles an unknown outcome with the next tree the store receives: a session still in the conversation
  // view drops the prefill, so a later switch by hand does not find a stale command typed in.
  useEffect(() => useTermStore.subscribe((state) => {
    const open = unresolved.current;
    if (!open || state.sessions === open.sessions) return;
    unresolved.current = null;
    const found = state.sessions.find((s) => s.id === session.id);
    if (found && (found.engine ?? "tui") !== "tui") clearPrefill(session.id, open.text);
  }), [session.id]);

  /** Move to `engine`; see `SwitchOptions`. A cancelled question calls none of the callbacks. */
  const switchTo = (engine: SessionEngine, opts?: SwitchOptions) => {
    // One switch at a time: a second request while one is running would ask a question it cannot answer.
    if (switching.current) return;
    pending.current = opts ?? {};
    if (status === "working") setAsking(engine);
    else apply(engine);
  };

  const confirm =
    failure !== null ? (
      <Backdrop onClose={() => setFailure(null)}>
        <div className="quit-card">
          <div className="quit-head">
            <div className="quit-title">{t("session.switchTitle")}</div>
            <div className="quit-body" role="alert">{failure}</div>
          </div>
          <div className="quit-foot">
            <button className="quit-btn primary" autoFocus onClick={() => setFailure(null)}>{t("common.close")}</button>
          </div>
        </div>
      </Backdrop>
    ) : asking === null ? null : (
      <Backdrop onClose={cancel}>
        <div
          className="quit-card"
          onKeyDown={(e) => {
            // A held Enter that opened this question from the composer must not also answer it.
            if (e.repeat) {
              e.preventDefault();
              return;
            }
            if (e.key === "Escape") cancel();
            if (e.key === "Enter") apply(asking);
          }}
        >
          <div className="quit-head">
            <div className="quit-title">{t("session.switchTitle")}</div>
            <div className="quit-body">{t("session.switchBody")}</div>
          </div>
          <div className="quit-foot">
            <button className="quit-btn ghost" onClick={cancel}>
              {t("common.cancel")}
            </button>
            <button className="quit-btn primary" autoFocus onClick={() => apply(asking)}>
              {t("session.switchConfirm")}
            </button>
          </div>
        </div>
      </Backdrop>
    );

  return { switchTo, confirm };
}
