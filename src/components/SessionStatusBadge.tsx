//! Textual session-status badge using the same state resolution and colors as the session tree and tab bar.
//! Shared by lists and dialogs that need to spell out the state name.

import { useT, type I18nKey } from "../i18n";
import { useTermStore } from "../store/termStore";
import { effectiveStatus, type DisplayStatus } from "../types";
import { StatusDot } from "./StatusDot";

const STATUS_LABEL_KEYS: Record<DisplayStatus, I18nKey> = {
  idle: "status.idle",
  running: "status.running",
  exited: "status.exited",
  error: "status.error",
  working: "status.working",
  asking: "status.asking",
  waiting: "status.waiting",
  awaiting_parent: "status.awaitingParent",
  awaiting_children: "status.awaitingChildren",
  background: "status.background",
  unavailable: "status.unavailable",
};

const STATUS_COLORS: Record<DisplayStatus, string> = {
  idle: "var(--status-exited)",
  running: "var(--status-running)",
  exited: "var(--status-exited)",
  error: "var(--status-error)",
  working: "var(--status-working)",
  asking: "var(--status-asking)",
  waiting: "var(--status-waiting)",
  awaiting_parent: "var(--status-workflow-wait)",
  awaiting_children: "var(--status-workflow-wait)",
  background: "var(--status-background)",
  unavailable: "var(--text-faint)",
};

export function SessionStatusBadge({ sessionId }: { sessionId: string }) {
  const t = useT();
  const status = useTermStore((s) => effectiveStatus(s.runtimes[sessionId]));
  const unread = useTermStore((s) => sessionId in s.notifications);
  // Match the left session tree's StatusIndicator: unread takes precedence over runtime state.
  const displayStatus = unread ? "asking" : status;
  const label = unread ? t("indicator.unread") : t(STATUS_LABEL_KEYS[status]);

  return (
    <span
      title={label}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 5,
        minWidth: 0,
        color: STATUS_COLORS[displayStatus],
        fontSize: 11,
        whiteSpace: "nowrap",
      }}
    >
      <StatusDot status={displayStatus} size={6} />
      <span style={{ overflow: "hidden", textOverflow: "ellipsis" }}>{label}</span>
    </span>
  );
}
