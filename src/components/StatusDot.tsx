//! Session status dot supporting both process lifecycle and agent activity states.

import type { DisplayStatus } from "../types";

const COLOR: Record<DisplayStatus, string> = {
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

export function StatusDot({
  status,
  size = 8,
}: {
  status: DisplayStatus;
  size?: number;
}) {
  const hollow = status === "awaiting_parent" || status === "awaiting_children";
  return (
    <span
      // Use a breathing animation for working and awaiting-action states so active work and user waits are recognizable
      // at a glance. Background tasks stay steady: they need no user action, and a pulse would read as a call to act.
      className={status === "working" || status === "asking" ? "vlx-status-pulse" : undefined}
      style={{
        width: size,
        height: size,
        borderRadius: "50%",
        boxSizing: hollow ? "border-box" : undefined,
        background: hollow ? "transparent" : COLOR[status],
        border: hollow ? `1.5px solid ${COLOR[status]}` : undefined,
        display: "inline-block",
        flex: "0 0 auto",
      }}
    />
  );
}
