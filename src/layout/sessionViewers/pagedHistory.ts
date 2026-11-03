import type { Session } from "../../types";

export function hasPagedHistoryKind(kind: Session["kind"]) {
  return kind === "claude" || kind === "codex" || kind === "pi" || kind === "omp" || kind === "antigravity" || kind === "opencode" || kind === "kiro" || kind === "grok" || kind === "deepseek";
}

export function hasPagedHistory(session: Pick<Session,"kind" | "agentSessionId">) {
  return hasPagedHistoryKind(session.kind) && Boolean(session.agentSessionId);
}
