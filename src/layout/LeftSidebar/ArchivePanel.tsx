//! Archive browser with archived sessions on the left and session content on the right. Agent
//! sessions render their JSONL transcript as readable, searchable, copyable chat; plain terminal
//! sessions or missing transcripts fall back to read-only recording playback. Open it from the
//! archive button in LeftSidebar. Each row can restore the session or delete it and its recording.
//!
//! The header performs full-text search within archives (`scope=archived`):
//! - An empty query shows every archived session and its complete content.
//! - A query reuses the global two-pane SearchConsole (see ../GlobalSearch/searchKit), with a result
//!   tree and restore/export/delete actions on group headers, match counts, and synchronized preview.
//!
//! Viewers and metadata helpers live in `../sessionViewers/` and are shared with global search.

import { useEffect, useMemo, useState } from "react";
import { Backdrop } from "../../components/Backdrop";

import Icons from "../../components/Icons";
import { canExportContext, exportSessionToFile } from "../../exportSession";
import { useT } from "../../i18n";
import { useTermStore } from "../../store/termStore";
import { SearchConsole, summarizeResults, useContentSearch, useSearchNav } from "../GlobalSearch/searchKit";
import { SessionContentViewer } from "../sessionViewers/SessionContentViewer";
import { fmtArchivedAt, KindIcon, locationOf } from "../sessionViewers/sessionMeta";

export function ArchivePanel() {
  const t = useT();
  const archivedSessions = useTermStore((s) => s.archivedSessions);
  const restoreSession = useTermStore((s) => s.restoreSession);
  const deleteNode = useTermStore((s) => s.deleteNode);
  const loadArchived = useTermStore((s) => s.loadArchived);
  const setArchiveOpen = useTermStore((s) => s.setArchiveOpen);
  const projects = useTermStore((s) => s.projects);
  const groups = useTermStore((s) => s.groups);
  const liveSessions = useTermStore((s) => s.sessions);

  // Full-text archive search; the backend constrains `scope=archived` at the SQL layer.
  const { query, setQuery, results, loading } = useContentSearch("archived");
  const searching = query.trim().length > 0;
  const nav = useSearchNav(results);
  const { totalMatches, sessions } = summarizeResults(results);

  // Archived session selected in browsing mode (empty query).
  const [selectedId, setSelectedId] = useState<string | null>(null);

  // Map archived session IDs to Sessions for the search console's locate viewer.
  const sessionById = useMemo(() => {
    const m = new Map<string, (typeof archivedSessions)[number]>();
    for (const s of archivedSessions) m.set(s.id, s);
    return m;
  }, [archivedSessions]);

  // In browsing mode, select the first archive when the current selection is absent or removed.
  useEffect(() => {
    if (searching) return;
    if (archivedSessions.length === 0) {
      setSelectedId(null);
      return;
    }
    if (!selectedId || !archivedSessions.some((s) => s.id === selectedId)) {
      setSelectedId(archivedSessions[0].id);
    }
  }, [searching, archivedSessions, selectedId]);

  const selected = archivedSessions.find((s) => s.id === selectedId) ?? null;
  const close = () => setArchiveOpen(false);

  // Per-session restore, export, and delete actions. Search mode places them on group headers;
  // browsing mode places them on rows in the left list.
  const sessionActions = (s: (typeof archivedSessions)[number]) => (
    <>
      <button
        className="icon-btn sm"
        title={t("archive.restore")}
        style={{ flex: "0 0 auto" }}
        onClick={(e) => {
          e.stopPropagation();
          void restoreSession(s.id);
        }}
      >
        <Icons.restart size={13} />
      </button>
      {canExportContext(s) && (
        <button
          className="icon-btn sm"
          title={t("archive.export")}
          style={{ flex: "0 0 auto" }}
          onClick={(e) => {
            e.stopPropagation();
            void exportSessionToFile(s);
          }}
        >
          <Icons.download size={13} />
        </button>
      )}
      <button
        className="icon-btn sm"
        title={t("archive.deleteForever")}
        style={{ flex: "0 0 auto", color: "var(--status-error)" }}
        onClick={(e) => {
          e.stopPropagation();
          void deleteNode("session", s.id).then(() => loadArchived());
        }}
      >
        <Icons.trash size={13} />
      </button>
    </>
  );

  const emptyHint = loading ? t("search.searching") : t("search.noResults");

  return (
    <Backdrop onClose={close}>
      <div
        style={{
          width: "82vw",
          height: "82vh",
          maxWidth: 1200,
          background: "var(--bg-panel)",
          border: "1px solid var(--border)",
          borderRadius: 10,
          boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header: title, archive query, totals, refresh, and close. */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            padding: "12px 16px",
            borderBottom: "1px solid var(--border)",
          }}
        >
          <span style={{ color: "var(--accent)" }}>
            <Icons.archive size={16} />
          </span>
          <span style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)", flex: "0 0 auto" }}>
            {t("archive.title")}
            {archivedSessions.length > 0 ? ` · ${archivedSessions.length}` : ""}
          </span>
          <div className="box" style={{ display: "flex", alignItems: "center", gap: 6, flex: 1 }}>
            <Icons.search size={14} style={{ color: "var(--text-muted)", flex: "0 0 auto" }} />
            <input
              placeholder={t("archive.searchPlaceholder")}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  if (query) setQuery("");
                  else close();
                } else if (searching && e.key === "ArrowDown") {
                  e.preventDefault();
                  nav.next();
                } else if (searching && e.key === "ArrowUp") {
                  e.preventDefault();
                  nav.prev();
                } else if (searching && e.key === "Enter") {
                  e.preventDefault();
                  if (e.shiftKey) nav.prev();
                  else nav.next();
                }
              }}
              style={{
                flex: 1,
                background: "transparent",
                border: "none",
                outline: "none",
                color: "var(--text-primary)",
                fontSize: 13,
              }}
            />
          </div>
          {searching && (
            <span style={{ fontSize: 12, color: "var(--text-muted)", flex: "0 0 auto" }}>
              {loading ? t("search.searching") : t("search.summary", totalMatches, sessions)}
            </span>
          )}
          <button className="icon-btn sm" title={t("common.refresh")} onClick={() => void loadArchived()}>
            <Icons.restart size={14} />
          </button>
          <button className="icon-btn sm" title={t("common.close")} onClick={close}>
            <Icons.x size={14} />
          </button>
        </div>

        <div
          style={{
            padding: "8px 16px",
            borderBottom: "1px solid var(--border)",
            fontSize: 12,
            color: "var(--text-muted)",
            lineHeight: 1.5,
          }}
        >
          {t("archive.retiredNote")}
        </div>

        {/* Search mode uses the two-pane console; browsing mode uses a list and full-content viewer. */}
        {searching ? (
          <SearchConsole
            nav={nav}
            results={results}
            query={query}
            sessionById={sessionById}
            projects={projects}
            groups={groups}
            liveSessions={liveSessions}
            renderGroupActions={(_hit, sess) => (sess ? sessionActions(sess) : null)}
            emptyHint={emptyHint}
          />
        ) : (
          <div style={{ flex: 1, display: "flex", minHeight: 0 }}>
            <div
              style={{
                width: 280,
                flex: "0 0 280px",
                borderRight: "1px solid var(--border)",
                overflowY: "auto",
              }}
            >
              {archivedSessions.length === 0 ? (
                <div style={{ padding: 20, fontSize: 13, color: "var(--text-muted)", lineHeight: 1.6 }}>
                  {t("archive.empty1")}
                  <br />
                  {t("archive.empty2")}
                </div>
              ) : (
                archivedSessions.map((s) => {
                  const active = s.id === selectedId;
                  const loc = locationOf(s, projects, groups, liveSessions);
                  return (
                    <div
                      key={s.id}
                      onClick={() => setSelectedId(s.id)}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: 8,
                        padding: "8px 12px",
                        cursor: "pointer",
                        borderBottom: "1px solid var(--border)",
                        background: active ? "var(--bg-hover)" : "transparent",
                      }}
                    >
                      <span style={{ color: "var(--text-muted)", flex: "0 0 auto" }}>
                        <KindIcon session={s} />
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
                          {s.name}
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
                        <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
                          {fmtArchivedAt(s.archivedAt)}
                        </div>
                      </div>
                      {sessionActions(s)}
                    </div>
                  );
                })
              )}
            </div>

            <div style={{ flex: 1, position: "relative", minWidth: 0 }}>
              {selected ? (
                <SessionContentViewer key={selected.id} session={selected} />
              ) : (
                <div
                  style={{
                    position: "absolute",
                    inset: 0,
                    display: "flex",
                    alignItems: "center",
                    justifyContent: "center",
                    color: "var(--text-muted)",
                    fontSize: 13,
                  }}
                >
                  {t("archive.pickOne")}
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </Backdrop>
  );
}
