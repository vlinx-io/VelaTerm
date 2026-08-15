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
import { platform } from "../platform";
import { useTermStore } from "../store/termStore";
import { Backdrop } from "./Backdrop";

export function QuitConfirmModal() {
  const t = useT();
  const saveWorkspaceOnQuit = useTermStore((s) => s.saveWorkspaceOnQuit);
  const setSaveWorkspaceOnQuit = useTermStore((s) => s.setSaveWorkspaceOnQuit);
  const saveWorkspaceSnapshot = useTermStore((s) => s.saveWorkspaceSnapshot);

  const [asking, setAsking] = useState(false);
  const [save, setSave] = useState(saveWorkspaceOnQuit);

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
    setAsking(false);
    void platform.quit.cancel();
  };

  const confirm = () => {
    setSaveWorkspaceOnQuit(save);
    // Write the snapshot before approving the exit; the shell terminates the process as soon as confirm resolves.
    if (save) saveWorkspaceSnapshot();
    setAsking(false);
    void platform.quit.confirm();
  };

  return (
    <Backdrop onClose={cancel}>
      <div
        style={{
          width: 400,
          background: "var(--bg-2)",
          border: "1px solid var(--border-strong)",
          borderRadius: 12,
          boxShadow: "var(--shadow)",
          overflow: "hidden",
        }}
        onKeyDown={(e) => {
          if (e.key === "Escape") cancel();
          if (e.key === "Enter") confirm();
        }}
      >
        <div style={{ padding: "14px 16px 10px" }}>
          <div style={{ fontWeight: 700, fontSize: 14, color: "var(--text)" }}>
            {t("quit.title")}
          </div>
          <div style={{ marginTop: 6, fontSize: 12, lineHeight: 1.6, color: "var(--text-dim)" }}>
            {t("quit.body")}
          </div>
        </div>

        <label
          style={{
            display: "flex",
            alignItems: "flex-start",
            gap: 8,
            margin: "0 12px 4px",
            padding: "8px 10px",
            borderRadius: 7,
            cursor: "pointer",
            background: save
              ? "var(--accent-dim, rgba(var(--accent-rgb, 63,207,142), 0.12))"
              : "transparent",
            border: save ? "1px solid var(--accent)" : "1px solid transparent",
            transition: "background 0.1s, border-color 0.1s",
          }}
        >
          <input
            type="checkbox"
            checked={save}
            onChange={(e) => setSave(e.target.checked)}
            style={{
              width: 16,
              height: 16,
              flex: "none",
              margin: "1px 0 0",
              accentColor: "var(--accent)",
              cursor: "pointer",
            }}
          />
          <span style={{ display: "flex", flexDirection: "column", gap: 3 }}>
            <span style={{ fontSize: 12.5, color: "var(--text)" }}>
              {t("quit.saveWorkspace")}
            </span>
            <span style={{ fontSize: 11.5, lineHeight: 1.5, color: "var(--text-dim)" }}>
              {t("quit.saveWorkspaceHint")}
            </span>
          </span>
        </label>

        <div
          style={{
            display: "flex",
            justifyContent: "flex-end",
            gap: 8,
            padding: "10px 16px",
            borderTop: "1px solid var(--border)",
          }}
        >
          <button
            onClick={cancel}
            style={{
              padding: "7px 14px",
              border: "1px solid var(--border)",
              borderRadius: 7,
              background: "transparent",
              color: "var(--text-dim)",
              fontSize: 12.5,
              cursor: "pointer",
            }}
          >
            {t("common.cancel")}
          </button>
          <button
            autoFocus
            onClick={confirm}
            style={{
              padding: "7px 16px",
              border: "none",
              borderRadius: 7,
              background: "var(--accent)",
              color: "var(--bg-0)",
              fontSize: 12.5,
              fontWeight: 600,
              cursor: "pointer",
            }}
          >
            {t("quit.confirm")}
          </button>
        </div>
      </div>
    </Backdrop>
  );
}
