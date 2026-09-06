//! Agent transcript viewer for scrollable, searchable, copyable messages parsed by the backend.
//!
//! Two caller-selected modes:
//! - Archive browsing uses an internal query to filter messages and shows a message count.
//! - Global or archive search passes `scrollToMessageIndex`, displays every message, and scrolls to
//!   the target. It omits the internal header because the outer search console already owns search
//!   and match navigation. `initialQuery` controls highlighting; the target uses active styling and
//!   other matches use regular styling (see highlight.tsx).

import { useEffect, useMemo, useRef, useState } from "react";

import Icons from "../../components/Icons";
import { dateLocale, useT } from "../../i18n";
import type { TranscriptMessage } from "../../ipc/commands";
import type { Session } from "../../types";
import { highlightMatches } from "./highlight";

/** Assistant display name based on session type. */
export function assistantLabel(kind: Session["kind"]): string {
  if (kind === "codex") return "Codex";
  if (kind === "opencode") return "OpenCode";
  if (kind === "copilot") return "Copilot";
  if (kind === "cursor") return "Cursor";
  if (kind === "antigravity") return "Antigravity";
  if (kind === "cline") return "Cline";
  if (kind === "pi") return "Pi";
  if (kind === "omp") return "OMP";
  if (kind === "crush") return "Crush";
  if (kind === "kimi") return "Kimi Code (K3)";
  if (kind === "kiro") return "Kiro";
  if (kind === "grok") return "Grok Build (Grok 4.5)";
  if (kind === "zoo") return "Zoo Code";
  return "Claude";
}

/** Convert an ISO timestamp to local time, returning an empty string on parse failure. */
export function fmtTs(ts?: string | null): string {
  if (!ts) return "";
  const d = new Date(ts);
  return Number.isNaN(d.getTime()) ? "" : d.toLocaleString(dateLocale());
}

export function TranscriptViewer({
  session,
  messages,
  initialQuery,
  highlightTerms,
  scrollToMessageIndex,
}: {
  session: Session;
  messages: TranscriptMessage[];
  /** Initial browsing query; also the locate-mode highlight when `highlightTerms` is absent. */
  initialQuery?: string;
  /** Locate mode: literals the search index matched, highlighted instead of the raw query. */
  highlightTerms?: string[];
  /** Message index to scroll to and highlight; providing it enables locate mode. */
  scrollToMessageIndex?: number;
}) {
  const t = useT();
  const label = assistantLabel(session.kind);
  const isLocate = scrollToMessageIndex != null;
  const targetRef = useRef<HTMLDivElement | null>(null);

  // Internal query for browsing mode; locate mode is controlled by highlightTerms/initialQuery.
  const [browseQuery, setBrowseQuery] = useState(initialQuery ?? "");
  // Effective highlight terms: the matched literals in locate mode, the internal input otherwise.
  const terms: string[] = isLocate
    ? (highlightTerms ?? [initialQuery ?? ""]).map((s) => s.trim()).filter(Boolean)
    : [browseQuery.trim()].filter(Boolean);

  // Locate mode preserves all messages and original indices; browsing mode filters by its query.
  const visible = useMemo(() => {
    const all = messages.map((m, idx) => ({ m, idx }));
    if (isLocate) return all;
    const ql = browseQuery.trim().toLowerCase();
    if (!ql) return all;
    return all.filter(({ m }) => m.text.toLowerCase().includes(ql));
  }, [messages, browseQuery, isLocate]);

  // Center the target message after mounting or changing the locate target.
  useEffect(() => {
    if (!isLocate) return;
    targetRef.current?.scrollIntoView({ block: "center" });
  }, [isLocate, scrollToMessageIndex, messages]);

  return (
    <div style={{ position: "absolute", inset: 0, display: "flex", flexDirection: "column" }}>
      {/* Browsing mode shows the query and message count. Locate mode delegates search and navigation to its outer console. */}
      {!isLocate && (
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 6,
            padding: "6px 8px",
            borderBottom: "1px solid var(--border)",
          }}
        >
          <div className="box" style={{ display: "flex", alignItems: "center", gap: 6, flex: 1 }}>
            <Icons.search size={13} />
            <input
              placeholder={t("archive.searchTranscript")}
              value={browseQuery}
              onChange={(e) => setBrowseQuery(e.target.value)}
              style={{
                flex: 1,
                background: "transparent",
                border: "none",
                outline: "none",
                color: "var(--text-primary)",
                fontSize: 12.5,
              }}
            />
          </div>
          <span style={{ fontSize: 11.5, color: "var(--text-muted)", flex: "0 0 auto" }}>
            {browseQuery.trim()
              ? t("archive.msgCountFiltered", visible.length, messages.length)
              : t("archive.msgCountAll", messages.length)}
          </span>
        </div>
      )}

      <div style={{ flex: 1, overflowY: "auto", padding: "10px 16px" }}>
        {visible.map(({ m, idx }) => {
          const isUser = m.role === "user";
          const isTarget = isLocate && idx === scrollToMessageIndex;
          return (
            <div
              key={idx}
              ref={isTarget ? targetRef : undefined}
              style={{
                margin: "10px 0",
                padding: "8px 12px",
                borderRadius: 8,
                background: isUser ? "var(--bg-hover)" : "transparent",
                border: isUser ? "none" : "1px solid var(--border)",
                borderLeft: isUser ? "3px solid var(--accent)" : "1px solid var(--border)",
                // Give the located message a prominent outline.
                boxShadow: isTarget ? "0 0 0 2px var(--accent)" : undefined,
              }}
            >
              <div style={{ display: "flex", alignItems: "baseline", gap: 8, marginBottom: 4 }}>
                <span
                  style={{
                    fontSize: 12,
                    fontWeight: 600,
                    color: isUser ? "var(--accent)" : "var(--text-primary)",
                  }}
                >
                  {isUser ? t("archive.you") : label}
                </span>
                <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{fmtTs(m.timestamp)}</span>
              </div>
              {m.text && (
                <div
                  style={{
                    fontSize: 13,
                    lineHeight: 1.65,
                    color: "var(--text-primary)",
                    whiteSpace: "pre-wrap",
                    wordBreak: "break-word",
                    userSelect: "text",
                  }}
                >
                  {/* Use active highlighting for the target and regular highlighting for other matches. */}
                  {terms.length > 0 ? highlightMatches(m.text, terms, isTarget) : m.text}
                </div>
              )}
              {m.tools.length > 0 && (
                <div style={{ fontSize: 11.5, color: "var(--text-muted)", marginTop: 6 }}>
                  {t("archive.toolsUsed", m.tools.join(" · "))}
                </div>
              )}
            </div>
          );
        })}
        {visible.length === 0 && (
          <div style={{ padding: 20, fontSize: 13, color: "var(--text-muted)" }}>
            {browseQuery.trim() ? t("archive.noMatch") : t("archive.emptyTranscript")}
          </div>
        )}
      </div>
    </div>
  );
}
