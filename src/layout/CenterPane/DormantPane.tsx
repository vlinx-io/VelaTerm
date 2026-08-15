//! Placeholder for a session restored from a saved workspace that has no process yet.
//!
//! Mounting `TerminalView` always spawns a PTY, so restoring a workspace by mounting every leaf would launch all
//! of its shells at once — slow at startup and rarely what the user wants after reopening the app. CenterPane
//! renders this card instead; activating it clears the dormant mark, which remounts the leaf as a real terminal.

import Icons from "../../components/Icons";
import { useT } from "../../i18n";
import { useTermStore } from "../../store/termStore";
import type { Session } from "../../types";

export function DormantPane({
  session,
  area,
  hidden,
  onActivate,
}: {
  session: Session;
  /** Absolute percentage rectangle from the pane layout, matching TerminalView. */
  area: React.CSSProperties;
  hidden: boolean;
  /** Focus the owning pane, keeping click-to-focus behavior identical to a live terminal. */
  onActivate?: () => void;
}) {
  const t = useT();
  const wakeSession = useTermStore((s) => s.wakeSession);

  const start = () => {
    onActivate?.();
    wakeSession(session.id);
  };

  return (
    <div
      style={{
        position: "absolute",
        ...area,
        display: hidden ? "none" : "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 16,
        background: "var(--bg-0)",
        overflow: "hidden",
      }}
      onMouseDown={() => onActivate?.()}
    >
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: 10,
          maxWidth: 320,
          textAlign: "center",
        }}
      >
        <div
          style={{
            fontSize: 13,
            fontWeight: 600,
            color: "var(--text)",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            maxWidth: "100%",
          }}
        >
          {session.name}
        </div>
        <div style={{ fontSize: 12, lineHeight: 1.6, color: "var(--text-dim)" }}>
          {t("dormant.body")}
        </div>
        <button
          onClick={start}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 6,
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
          <Icons.terminal size={14} />
          {t("dormant.start")}
        </button>
      </div>
    </div>
  );
}
