//! Dismissible notice shown when the Electron desktop shell automatically restarted the local
//! background service after an unexpected exit. Stays visible until dismissed so the user sees it
//! even if they return to the window later.

import { useEffect, useState } from "react";
import { useT } from "../i18n";
import { platform } from "../platform";

export function ServiceRestartNotice() {
  const tt = useT();
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    let active = true;
    void platform.window
      .takeServiceRestartNotice()
      .then((restarted) => {
        if (active && restarted) setVisible(true);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  if (!visible) return null;
  return (
    <div
      role="alert"
      style={{
        position: "fixed",
        top: 44,
        left: "50%",
        transform: "translateX(-50%)",
        zIndex: 9998,
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "8px 12px 8px 14px",
        borderRadius: 6,
        background: "var(--red, #c0392b)",
        color: "var(--bg-0)",
        fontSize: 12,
        maxWidth: "80%",
        boxShadow: "0 4px 16px rgba(0, 0, 0, 0.28)",
      }}
    >
      <span>{tt("service.restartedNotice")}</span>
      <button
        type="button"
        aria-label={tt("common.close")}
        onClick={() => setVisible(false)}
        style={{
          background: "transparent",
          border: "none",
          color: "inherit",
          cursor: "pointer",
          padding: "0 4px",
          fontSize: 14,
          lineHeight: 1,
          opacity: 0.9,
        }}
      >
        ×
      </button>
    </div>
  );
}
