//! Application-exit confirmation, replacing the shells' native message dialog.
//!
//! Both desktop shells intercept a user-triggered exit and hand the decision here (see platform QuitCapability),
//! because a native dialog supports neither the "save workspace" checkbox nor translated copy. The handshake is:
//! shell fires the request → this component acknowledges immediately so the shell's watchdog stands down → the
//! user decides → the snapshot is written first, then the exit is approved.
//!
//! The checkbox answer is remembered as the next launch's default, but the workspace itself is saved only for the
//! exit the user actually ticked.

import { useEffect, useState } from "react";
import { useT } from "../i18n";
import { flushNow } from "../ipc/settingsSync";
import { platform } from "../platform";
import { useTermStore } from "../store/termStore";
import { Backdrop } from "./Backdrop";

/** Upper bound on the pre-exit settings flush; see the comment in `confirm`. */
const FLUSH_TIMEOUT_MS = 600;

export function QuitConfirmModal() {
  const t = useT();
  const saveWorkspaceOnQuit = useTermStore((s) => s.saveWorkspaceOnQuit);
  const setSaveWorkspaceOnQuit = useTermStore((s) => s.setSaveWorkspaceOnQuit);
  const saveWorkspaceSnapshot = useTermStore((s) => s.saveWorkspaceSnapshot);

  const [asking, setAsking] = useState(false);
  const [save, setSave] = useState(saveWorkspaceOnQuit);
  /** Set once the exit is approved, to keep the in-flight settings flush from being re-entered. */
  const [exiting, setExiting] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    void platform.quit
      .onRequested(() => {
        // Seed from the remembered preference each time, then acknowledge so the shell keeps waiting for us.
        setSave(useTermStore.getState().saveWorkspaceOnQuit);
        setAsking(true);
        void platform.quit.ack();
      })
      .then((fn) => {
        if (!fn) return;
        if (disposed) fn();
        else unlisten = fn;
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  if (!asking) return null;

  const cancel = () => {
    if (exiting) return; // The exit is already approved; a backdrop click must not cancel it half-way.
    setAsking(false);
    void platform.quit.cancel();
  };

  const confirm = async () => {
    if (exiting) return;
    setExiting(true);
    setSaveWorkspaceOnQuit(save);
    // Force the checkbox answer to the backend before approving the exit. Settings writes are debounced by
    // 400 ms, and the process dies well inside that window, so the backend would keep the previous value.
    // Startup reconciliation treats the backend as authoritative and overwrites the local copy from it, which
    // means a skipped flush does not merely fail to save the answer -- it silently reverts the one already
    // written to localStorage, and the checkbox comes back unticked every launch.
    //
    // Cap the wait: `invoke` never times out, and over WebSocket transport a half-open socket settles the
    // request neither with a reply nor with a close, so an unbounded await would leave the dialog frozen with
    // no way to quit. The bound sits just above the debounce window it replaces. Losing the write is the
    // lesser failure -- localStorage keeps this shell correct, and reconciliation is only wrong when another
    // shell has since changed the same key.
    await flushNow(FLUSH_TIMEOUT_MS);
    // Write the snapshot before approving the exit; the shell terminates the process as soon as confirm resolves.
    if (save) saveWorkspaceSnapshot();
    setAsking(false);
    void platform.quit.confirm();
  };

  return (
    <Backdrop onClose={cancel}>
      <div
        className="quit-card"
        onKeyDown={(e) => {
          if (e.key === "Escape") cancel();
          if (e.key === "Enter") void confirm();
        }}
      >
        <div className="quit-head">
          <div className="quit-title">{t("quit.title")}</div>
          <div className="quit-body">{t("quit.body")}</div>
        </div>

        <label className={save ? "quit-opt on" : "quit-opt"}>
          <input
            type="checkbox"
            checked={save}
            onChange={(e) => setSave(e.target.checked)}
          />
          <span className="quit-box" aria-hidden="true">
            <svg width="10" height="10" viewBox="0 0 10 10" style={{ opacity: save ? 1 : 0 }}>
              <path
                d="M1.6 5.2 4 7.6 8.6 2.4"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.6"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </span>
          <span>
            <span className="quit-opt-label">{t("quit.saveWorkspace")}</span>
            <span className="quit-opt-hint" style={{ display: "block" }}>
              {t("quit.saveWorkspaceHint")}
            </span>
          </span>
        </label>

        <div className="quit-foot">
          <button className="quit-btn ghost" onClick={cancel}>
            {t("common.cancel")}
          </button>
          <button
            className="quit-btn primary"
            autoFocus
            disabled={exiting}
            onClick={() => void confirm()}
          >
            {t("quit.confirm")}
          </button>
        </div>
      </div>
    </Backdrop>
  );
}
