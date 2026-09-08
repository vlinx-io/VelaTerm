//! Moving a session between its two views, and the one question that move sometimes has to ask.
//!
//! The terminal view and the conversation view are not two readings of one process: the terminal runs the
//! agent's own interface, the conversation runs it as a protocol peer. Switching therefore stops one and
//! starts the other. The conversation survives — both engines write the same record and pick it back up —
//! but a turn in flight does not, so a working agent is asked about first and an idle one is simply moved.

import { useRef, useState } from "react";

import { Backdrop } from "../../../components/Backdrop";
import { useT } from "../../../i18n";
import { useTermStore } from "../../../store/termStore";
import { effectiveStatus, type Session, type SessionEngine } from "../../../types";

export function useEngineSwitch(session: Session) {
  const t = useT();
  const status = useTermStore((s) => effectiveStatus(s.runtimes[session.id]));
  // The engine waiting on an answer, or null when nothing is being asked.
  const [asking, setAsking] = useState<SessionEngine | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const switching = useRef(false);

  const apply = (engine: SessionEngine) => {
    if (switching.current) return;
    switching.current = true;
    setAsking(null);
    setFailure(null);
    void useTermStore
      .getState()
      .setSessionEngineMode(session.id, engine)
      .catch((error) => setFailure(String(error)))
      .finally(() => { switching.current = false; });
  };

  const switchTo = (engine: SessionEngine) => {
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
      <Backdrop onClose={() => setAsking(null)}>
        <div
          className="quit-card"
          onKeyDown={(e) => {
            if (e.key === "Escape") setAsking(null);
            if (e.key === "Enter") apply(asking);
          }}
        >
          <div className="quit-head">
            <div className="quit-title">{t("session.switchTitle")}</div>
            <div className="quit-body">{t("session.switchBody")}</div>
          </div>
          <div className="quit-foot">
            <button className="quit-btn ghost" onClick={() => setAsking(null)}>
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
