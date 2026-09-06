//! Shared knowledge-base search console used by GlobalSearch and ArchivePanel. Each panel supplies its own
//! overlay, header, and field, while this module owns the two-column body and search logic.
//!
//! - `useContentSearch(scope)` manages a controlled query, 250ms debounce, and request sequencing before
//!   calling searchSessionContent. Scope changes immediately repeat the search in the new range.
//! - `useSearchNav(results)` flattens hits into ordered FlatMatch entries and navigates continuously across
//!   sessions with arrow keys, Enter, or Shift+Enter.
//! - `summarizeResults(results)` computes actual total, session, and navigable hit counts.
//! - `SearchConsole` shows grouped hits with role/time/highlight on the left and a synchronized preview on
//!   the right. `useTranscriptCache` resolves missing role/time by messageIndex. Within one session, the
//!   preview remains mounted and cached and merely scrolls to the next hit.

import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";

import Icons from "../../components/Icons";
import { useT } from "../../i18n";
import {
  readAgentTranscript,
  searchSessionContent,
  type SearchMatch,
  type SearchScope,
  type SessionSearchHit,
  type TranscriptMessage,
} from "../../ipc/commands";
import type { Session } from "../../types";
import { highlightMatches } from "../sessionViewers/highlight";
import { SessionContentViewer } from "../sessionViewers/SessionContentViewer";
import { fmtTsShort, KindIcon, locationOf } from "../sessionViewers/sessionMeta";
import { assistantLabel } from "../sessionViewers/TranscriptViewer";

/** Search debounce interval in milliseconds. */
const DEBOUNCE_MS = 250;

/** Value returned by useContentSearch. */
export interface ContentSearch {
  query: string;
  setQuery: (q: string) => void;
  results: SessionSearchHit[];
  loading: boolean;
}

/**
 * Debounced content-search hook. Query or scope changes search the current range after 250ms, with request
 * sequence numbers preventing stale overwrites. Empty queries clear immediately without a request.
 */
export function useContentSearch(scope: SearchScope): ContentSearch {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SessionSearchHit[]>([]);
  const [loading, setLoading] = useState(false);
  const reqId = useRef(0);

  useEffect(() => {
    const q = query.trim();
    if (!q) {
      setResults([]);
      setLoading(false);
      return;
    }
    setLoading(true);
    const id = ++reqId.current;
    const handle = setTimeout(() => {
      searchSessionContent(q, scope)
        .then((r) => {
          if (id === reqId.current) {
            setResults(r);
            setLoading(false);
          }
        })
        .catch(() => {
          if (id === reqId.current) {
            setResults([]);
            setLoading(false);
          }
        });
    }, DEBOUNCE_MS);
    return () => clearTimeout(handle);
  }, [query, scope]);

  return { query, setQuery, results, loading };
}

/** Centered message used for empty states. */
export function CenterMessage({ text }: { text: string }) {
  return (
    <div
      style={{
        position: "absolute",
        inset: 0,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        color: "var(--text-muted)",
        fontSize: 13,
        padding: 20,
        textAlign: "center",
      }}
    >
      {text}
    </div>
  );
}

// ─────────────────────────── Hit flattening and navigation ───────────────────────────

/** One flattened hit with a global index in the complete result stream. */
export interface FlatMatch {
  /** Zero-based index in the complete hit stream. */
  globalIndex: number;
  sessionId: string;
  /** Owning session, used for name, kind, source, and archived state. */
  hit: SessionSearchHit;
  /** Underlying match containing messageIndex, ordinal, and snippet. */
  match: SearchMatch;
  /** Zero-based match index within its session. */
  indexInSession: number;
}

/** Flatten session results while preserving backend relevance order. */
export function flattenResults(results: SessionSearchHit[]): FlatMatch[] {
  const out: FlatMatch[] = [];
  for (const hit of results) {
    hit.matches.forEach((match, i) => {
      out.push({
        globalIndex: out.length,
        sessionId: hit.sessionId,
        hit,
        match,
        indexInSession: i,
      });
    });
  }
  return out;
}

/** Actual summary displayed at the top. */
export interface SearchSummary {
  /** Total matchCount sum, including hits omitted by the 50-item detail cap. */
  totalMatches: number;
  /** Number of matching sessions. */
  sessions: number;
  /** Navigable match count from matches.length, no greater than totalMatches. */
  locatable: number;
}

/** Summarize results for the host header and preview bar. */
export function summarizeResults(results: SessionSearchHit[]): SearchSummary {
  let totalMatches = 0;
  let locatable = 0;
  for (const hit of results) {
    totalMatches += hit.matchCount;
    locatable += hit.matches.length;
  }
  return { totalMatches, sessions: results.length, locatable };
}

/** Match navigation controller created by the host and passed into SearchConsole and field key handling. */
export interface SearchNav {
  flat: FlatMatch[];
  activeIndex: number;
  setActiveIndex: (i: number) => void;
  /** Next hit across sessions, wrapping at the end. */
  next: () => void;
  /** Previous hit across sessions, wrapping at the beginning. */
  prev: () => void;
}

/** Flatten results, maintain activeIndex, and navigate; reset to zero when results change. */
export function useSearchNav(results: SessionSearchHit[]): SearchNav {
  const flat = useMemo(() => flattenResults(results), [results]);
  const [activeIndex, setActiveIndex] = useState(0);
  useEffect(() => {
    setActiveIndex(0);
  }, [results]);
  const len = flat.length;
  const next = useCallback(() => {
    if (len > 0) setActiveIndex((i) => (i + 1) % len);
  }, [len]);
  const prev = useCallback(() => {
    if (len > 0) setActiveIndex((i) => (i - 1 + len) % len);
  }, [len]);
  return { flat, activeIndex, setActiveIndex, next, prev };
}

// ─────────────────────────── Transcript cache for role and time ───────────────────────────

/**
 * Transcript cache. Backend search hits provide only messageIndex, so lazily load each complete transcript
 * through readAgentTranscript and resolve role/time for left-tree rows. The preview reuses the same cache.
 */
function useTranscriptCache() {
  const [cache, setCache] = useState<Map<string, TranscriptMessage[]>>(() => new Map());
  const loaded = useRef<Set<string>>(new Set());
  const pending = useRef<Set<string>>(new Set());
  const ensure = useCallback((sessionId: string) => {
    if (loaded.current.has(sessionId) || pending.current.has(sessionId)) return;
    pending.current.add(sessionId);
    readAgentTranscript(sessionId)
      .then((msgs) => {
        loaded.current.add(sessionId);
        setCache((prev) => new Map(prev).set(sessionId, msgs));
      })
      .catch(() => {
        // Cache failures as loaded-and-empty to avoid retries; the preview then falls back to recording.
        loaded.current.add(sessionId);
        setCache((prev) => new Map(prev).set(sessionId, []));
      })
      .finally(() => {
        pending.current.delete(sessionId);
      });
  }, []);
  return { cache, ensure };
}

/** Resolve hit role/time: recordings use Terminal without time; transcripts look up messageIndex. */
function roleAndTime(
  fm: FlatMatch,
  cache: Map<string, TranscriptMessage[]>,
  t: ReturnType<typeof useT>,
): { role: string; ts: string } {
  if (fm.hit.source === "recording") {
    return { role: t("search.roleTerminal"), ts: "" };
  }
  const msgs = cache.get(fm.sessionId);
  const mi = fm.match.messageIndex;
  if (msgs && mi != null && mi >= 0 && mi < msgs.length) {
    const m = msgs[mi];
    return {
      role: m.role === "user" ? t("archive.you") : assistantLabel(fm.hit.kind),
      ts: fmtTsShort(m.timestamp),
    };
  }
  // Show only the snippet until the transcript loads, then fill role and time.
  return { role: "", ts: "" };
}

// ─────────────────────────── Two-column search console ───────────────────────────

/**
 * Knowledge-base search console shared by global and archive search. It fills the panel body with flex:1.
 */
export function SearchConsole({
  nav,
  results,
  sessionById,
  projects,
  groups,
  liveSessions,
  onOpenSession,
  renderGroupActions,
  emptyHint,
}: {
  /** Navigation controller created by the host with useSearchNav. */
  nav: SearchNav;
  results: SessionSearchHit[];
  /** Complete Session by ID for viewer selection and breadcrumb resolution. */
  sessionById: Map<string, Session>;
  projects: { id: string; name: string }[];
  groups: { id: string; name: string; parentGroupId?: string | null }[];
  liveSessions: Session[];
  /** When supplied, show Open Session for an active nonarchived match. */
  onOpenSession?: (hit: SessionSearchHit) => void;
  /** Optional group-header actions such as archive restore, export, and delete. */
  renderGroupActions?: (hit: SessionSearchHit, session: Session | null) => ReactNode;
  /** Centered empty-stream message selected by the host for blank, no-result, or searching states. */
  emptyHint?: ReactNode;
}) {
  const t = useT();
  const { flat, activeIndex, setActiveIndex, next, prev } = nav;
  const { cache, ensure } = useTranscriptCache();
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());
  const activeRowRef = useRef<HTMLDivElement | null>(null);

  const active = flat[activeIndex] ?? null;

  // When results change, cache transcripts for matching conversation sessions for both rows and preview.
  useEffect(() => {
    for (const hit of results) {
      if (hit.source === "transcript") ensure(hit.sessionId);
    }
  }, [results, ensure]);

  // Expand a collapsed session containing the active hit so its row remains visible.
  useEffect(() => {
    if (active && collapsed.has(active.sessionId)) {
      setCollapsed((prev) => {
        const n = new Set(prev);
        n.delete(active.sessionId);
        return n;
      });
    }
    // Consider automatic expansion only when activeIndex changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeIndex]);

  // Scroll the active row into view after index or collapse-state changes.
  useEffect(() => {
    activeRowRef.current?.scrollIntoView({ block: "nearest" });
  }, [activeIndex, collapsed]);

  // At document level, navigate hits with arrows/Enter outside inputs and textareas. The host search field
  // and recording viewer manage their own key handling.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement | null)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA") return;
      // Let a focused button handle Enter to avoid combining its click with two-hit navigation.
      if (e.key === "Enter" && tag === "BUTTON") return;
      if (e.key === "ArrowDown") {
        e.preventDefault();
        next();
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        prev();
      } else if (e.key === "Enter") {
        e.preventDefault();
        if (e.shiftKey) prev();
        else next();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [next, prev]);

  // Group hit rows by session while preserving result order.
  const grouped = useMemo(() => {
    const bySession = new Map<string, FlatMatch[]>();
    for (const fm of flat) {
      const arr = bySession.get(fm.sessionId);
      if (arr) arr.push(fm);
      else bySession.set(fm.sessionId, [fm]);
    }
    return results.map((hit) => ({ hit, items: bySession.get(hit.sessionId) ?? [] }));
  }, [flat, results]);

  const toggleCollapse = (sessionId: string) =>
    setCollapsed((prev) => {
      const n = new Set(prev);
      if (n.has(sessionId)) n.delete(sessionId);
      else n.add(sessionId);
      return n;
    });

  if (flat.length === 0) {
    return (
      <div
        style={{
          flex: 1,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          minHeight: 0,
          padding: 20,
          textAlign: "center",
          color: "var(--text-muted)",
          fontSize: 13,
          lineHeight: 1.6,
        }}
      >
        {emptyHint}
      </div>
    );
  }

  return (
    <div style={{ flex: 1, display: "flex", minHeight: 0 }}>
      {/* Left: the results tree. */}
      <div
        style={{
          width: 380,
          flex: "0 0 380px",
          borderRight: "1px solid var(--border)",
          overflowY: "auto",
        }}
      >
        {grouped.map(({ hit, items }) => {
          const sess = sessionById.get(hit.sessionId) ?? null;
          const loc = sess ? locationOf(sess, projects, groups, liveSessions) : "";
          const isCollapsed = collapsed.has(hit.sessionId);
          const firstMatchIndex = items[0]?.globalIndex;
          const selectSession = () => {
            if (firstMatchIndex === undefined) return;
            if (isCollapsed) toggleCollapse(hit.sessionId);
            setActiveIndex(firstMatchIndex);
          };
          return (
            <div key={hit.sessionId}>
              {/* Group header: collapse arrow, icon, name and location, hit count, archive badge and extra buttons. */}
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 6,
                  padding: "7px 10px",
                  borderBottom: "1px solid var(--border)",
                  background: "var(--bg-panel)",
                  position: "sticky",
                  top: 0,
                  zIndex: 1,
                }}
              >
                <button
                  type="button"
                  className="icon-btn sm"
                  title={isCollapsed ? t("search.expandGroup") : t("search.collapseGroup")}
                  onClick={(e) => {
                    e.stopPropagation();
                    toggleCollapse(hit.sessionId);
                  }}
                  style={{
                    width: 16,
                    height: 16,
                    minWidth: 16,
                    padding: 0,
                    color: "var(--text-muted)",
                    flex: "0 0 auto",
                  }}
                >
                  {isCollapsed ? <Icons.chevR size={13} /> : <Icons.chevD size={13} />}
                </button>
                <button
                  type="button"
                  onClick={selectSession}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 6,
                    flex: 1,
                    minWidth: 0,
                    padding: 0,
                    border: 0,
                    background: "transparent",
                    color: "inherit",
                    font: "inherit",
                    textAlign: "left",
                    cursor: "pointer",
                  }}
                >
                  <span style={{ color: "var(--text-muted)", flex: "0 0 auto", display: "inline-flex" }}>
                    {sess ? <KindIcon session={sess} /> : <Icons.bot size={14} />}
                  </span>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div
                      style={{
                        fontSize: 13,
                        color: "var(--text-primary)",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                    >
                      {hit.name}
                    </div>
                    {loc && (
                      <div
                        style={{
                          fontSize: 11,
                          color: "var(--text-muted)",
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                        }}
                        title={loc}
                      >
                        {loc}
                      </div>
                    )}
                  </div>
                  {hit.archived && (
                    <span
                      style={{
                        fontSize: 10,
                        color: "var(--text-muted)",
                        border: "1px solid var(--border)",
                        borderRadius: 4,
                        padding: "1px 4px",
                        flex: "0 0 auto",
                      }}
                    >
                      {t("search.archivedBadge")}
                    </span>
                  )}
                  <span
                    style={{ fontSize: 11, color: "var(--accent)", fontWeight: 600, flex: "0 0 auto" }}
                    title={t("search.matchCount", hit.matchCount)}
                  >
                    {hit.matchCount}
                  </span>
                </button>
                {renderGroupActions && (
                  <span
                    style={{ display: "inline-flex", gap: 2, flex: "0 0 auto" }}
                    onClick={(e) => e.stopPropagation()}
                  >
                    {renderGroupActions(hit, sess)}
                  </span>
                )}
              </div>

              {/* A single hit row. */}
              {!isCollapsed &&
                items.map((fm) => {
                  const isActive = fm.globalIndex === activeIndex;
                  const { role, ts } = roleAndTime(fm, cache, t);
                  return (
                    <div
                      key={fm.globalIndex}
                      ref={isActive ? activeRowRef : undefined}
                      onClick={() => setActiveIndex(fm.globalIndex)}
                      style={{
                        padding: "6px 10px 6px 28px",
                        cursor: "pointer",
                        borderBottom: "1px solid var(--border)",
                        background: isActive ? "var(--bg-hover)" : "transparent",
                        borderLeft: isActive ? "2px solid var(--accent)" : "2px solid transparent",
                      }}
                    >
                      <div style={{ display: "flex", alignItems: "baseline", gap: 6, marginBottom: 2 }}>
                        <span
                          style={{
                            fontSize: 11,
                            fontWeight: 600,
                            color: isActive ? "var(--accent)" : "var(--text-muted)",
                          }}
                        >
                          {role || "·"}
                        </span>
                        {ts && <span style={{ fontSize: 10.5, color: "var(--text-muted)" }}>{ts}</span>}
                      </div>
                      <div
                        style={{
                          fontSize: 12,
                          lineHeight: 1.5,
                          color: "var(--text-primary)",
                          wordBreak: "break-word",
                          display: "-webkit-box",
                          WebkitLineClamp: 2,
                          WebkitBoxOrient: "vertical",
                          overflow: "hidden",
                        }}
                      >
                        {highlightMatches(fm.match.snippet, fm.match.matched)}
                      </div>
                    </div>
                  );
                })}
            </div>
          );
        })}
      </div>

      {/* Right: the linked preview. */}
      <div style={{ flex: 1, display: "flex", flexDirection: "column", minWidth: 0 }}>
        {/* Preview navigation bar: N of M, previous and next, plus Open session for a live session. */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            padding: "6px 10px",
            borderBottom: "1px solid var(--border)",
          }}
        >
          <span style={{ fontSize: 12, color: "var(--text-muted)", flex: "0 0 auto" }}>
            {t("search.matchPosition", activeIndex + 1, flat.length)}
          </span>
          <button className="icon-btn sm" title={t("common.prev")} onClick={prev}>
            <Icons.chevD size={14} style={{ transform: "rotate(180deg)" }} />
          </button>
          <button className="icon-btn sm" title={t("common.next")} onClick={next}>
            <Icons.chevD size={14} />
          </button>
          <div style={{ flex: 1 }} />
          {onOpenSession && active && !active.hit.archived && (
            <button
              className="icon-btn sm"
              title={t("search.openSession")}
              onClick={() => onOpenSession(active.hit)}
              style={{ width: "auto", padding: "2px 8px", gap: 4, display: "flex", alignItems: "center" }}
            >
              <Icons.terminal size={13} />
              <span style={{ fontSize: 12 }}>{t("search.openSession")}</span>
            </button>
          )}
        </div>

        {/* Preview body. */}
        <div style={{ flex: 1, position: "relative", minHeight: 0 }}>
          <Preview active={active} sessionById={sessionById} cache={cache} />
        </div>
      </div>
    </div>
  );
}

/** Right-side preview keyed only by sessionId so navigating within a session does not remount it. Memoization
 * protects the console's heaviest transcript highlighting/recording-seek work; while the user types, results
 * and therefore `active` stay unchanged until the debounced request lands, so rerenders are skipped. The
 * highlighted literals come from the active hit itself (SearchMatch.matched), not from the raw query. */
const Preview = memo(function Preview({
  active,
  sessionById,
  cache,
}: {
  active: FlatMatch | null;
  sessionById: Map<string, Session>;
  cache: Map<string, TranscriptMessage[]>;
}) {
  const t = useT();
  if (!active) return <CenterMessage text={t("search.pickSession")} />;
  const sess = sessionById.get(active.sessionId) ?? null;
  if (!sess) return <CenterMessage text={t("search.pickSession")} />;

  // Recording hits open replay and repeat findNext by ordinal; RecordingViewer seeks when ordinal changes.
  if (active.hit.source === "recording") {
    return (
      <SessionContentViewer
        key={active.sessionId}
        session={sess}
        source="recording"
        highlightTerms={active.match.matched}
        scrollToOrdinal={active.match.ordinal}
      />
    );
  }

  // Transcript hits render cached records, showing loading until available.
  const msgs = cache.get(active.sessionId);
  if (msgs === undefined) {
    return <CenterMessage text={t("archive.loadingTranscript")} />;
  }
  return (
    <SessionContentViewer
      key={active.sessionId}
      session={sess}
      source="transcript"
      highlightTerms={active.match.matched}
      preloadedMessages={msgs}
      scrollToMessageIndex={active.match.messageIndex ?? undefined}
      scrollToOrdinal={active.match.ordinal}
    />
  );
});
