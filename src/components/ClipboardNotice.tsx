//! Brief notice that a paste was abandoned because the clipboard source is not responding. App mounts exactly
//! one global instance.

import { useEffect, useState } from "react";
import { useT } from "../i18n";
import { onClipboardUnresponsive } from "../platform/clipboardStall";

const VISIBLE_MS = 5000;

export function ClipboardNotice() {
  const tt = useT();
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const unsubscribe = onClipboardUnresponsive(() => {
      setVisible(true);
      clearTimeout(timer);
      timer = setTimeout(() => setVisible(false), VISIBLE_MS);
    });
    return () => {
      unsubscribe();
      clearTimeout(timer);
    };
  }, []);

  if (!visible) return null;
  return (
    <div
      role="status"
      style={{
        position: "fixed",
        top: 44,
        left: "50%",
        transform: "translateX(-50%)",
        zIndex: 9999,
        padding: "6px 14px",
        borderRadius: 6,
        background: "var(--red, #c0392b)",
        color: "var(--bg-0)",
        fontSize: 12,
        pointerEvents: "none",
        maxWidth: "80%",
      }}
    >
      {tt("clipboard.unresponsive")}
    </div>
  );
}
