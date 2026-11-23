//! Global session-content search overlay for searching session history in a two-pane console.
//! The header contains the query, a project/collection filter, an "include archived" toggle, and
//! actual result totals. The shared SearchConsole renders a result tree on the left and a
//! synchronized preview on the right. Open it with Cmd+Shift+F or the search button in the sidebar.
//!
//! Search scope (see IPC SearchScope): live sessions only by default (`scope=live`). Enabling
//! "include archived" switches to `scope=all`. Selecting a project or collection restricts the
//! backend SQL query to that project or to the collection and all of its nested projects and
//! sub-collections.
//!
//! The read-only preview scrolls to and highlights the active match for any session. Live,
//! unarchived sessions also offer an "open session" action, followed by `registry.findNext` for
//! an active terminal when possible.

import { useEffect, useMemo, useRef, useState } from "react";
import { Backdrop } from "../../components/Backdrop";

import Icons from "../../components/Icons";
import Select, { type SelectOption } from "../../components/Select";
import { useT } from "../../i18n";
import type { SessionSearchHit } from "../../ipc/commands";
import { useTermStore } from "../../store/termStore";
import { findNext } from "../../terminal/registry";
import { isVirtualProject, type Session } from "../../types";
import { arrangeProjectsInCollections } from "../LeftSidebar/projectCollectionLayout";
import { projectDisplayName, SystemCollectionIcon, systemSection } from "../LeftSidebar/systemCollections";
import { ContentSearchControls, SearchConsole, summarizeResults, useContentSearch, useSearchNav } from "./searchKit";

export function GlobalSearch() {
  const t = useT();
  const setGlobalSearchOpen = useTermStore((s) => s.setGlobalSearchOpen);
  const loadArchived = useTermStore((s) => s.loadArchived);
  const openSession = useTermStore((s) => s.openSession);
  const projects = useTermStore((s) => s.projects);
  const groups = useTermStore((s) => s.groups);
  const liveSessions = useTermStore((s) => s.sessions);
  const archivedSessions = useTermStore((s) => s.archivedSessions);
  const runtimes = useTermStore((s) => s.runtimes);

  // Archived sessions are excluded and all projects/collections are searched by default.
  // Remounting the panel resets both filters.
  const [includeArchived, setIncludeArchived] = useState(false);
  const [projectId, setProjectId] = useState("");
  const activeProject = useMemo(
    () => (projectId ? projects.find((p) => p.id === projectId) ?? null : null),
    [projectId, projects],
  );
  const activeProjectId = activeProject?.id ?? "";
  const search = useContentSearch(includeArchived ? "all" : "live", undefined, activeProjectId || undefined);
  const { query, setQuery, results, loading } = search;
  const nav = useSearchNav(results);
  const inputRef = useRef<HTMLInputElement | null>(null);

  // Build hierarchical filter options matching the sidebar tree: built-in collections, user
  // collections, and folder projects.
  const locationOptions = useMemo<SelectOption<string>[]>(() => {
    const entries = arrangeProjectsInCollections(projects, () => true);
    const items: SelectOption<string>[] = [{ value: "", label: t("search.allLocations") }];
    entries.forEach(({ project, indent }, index) => {
      const section = systemSection(project.id);
      const isCollection = isVirtualProject(project);
      items.push({
        value: project.id,
        label: projectDisplayName(t, project),
        icon: section ? (
          <SystemCollectionIcon section={section} size={13} />
        ) : isCollection ? (
          <Icons.collection size={13} />
        ) : (
          <Icons.project size={13} />
        ),
        hint: !section && isCollection ? t("newSession.collectionOption") : undefined,
        indent,
        separatorBefore: index === 0,
      });
    });
    return items;
  }, [projects, t]);
  const selectedLocationLabel = activeProject ? projectDisplayName(t, activeProject) : t("search.allLocations");

  // Load archived sessions on open so archived matches resolve to complete Session objects.
  useEffect(() => {
    void loadArchived();
    inputRef.current?.focus();
  }, [loadArchived]);

  // Map session IDs to complete Session objects from either live or archived sessions.
  const sessionById = useMemo(() => {
    const m = new Map<string, Session>();
    for (const s of liveSessions) m.set(s.id, s);
    for (const s of archivedSessions) if (!m.has(s.id)) m.set(s.id, s);
    return m;
  }, [liveSessions, archivedSessions]);

  const close = () => setGlobalSearchOpen(false);
  const { locatable } = summarizeResults(results);
  const totalMatches=search.page?.totalMatches ?? 0;
  const sessions=search.page?.totalSessions ?? 0;

  // For a live terminal session, try to jump to the match; its 5,000-line scrollback may not contain it.
  const isLiveTerminal = (hit: SessionSearchHit) =>
    !hit.archived && hit.kind === "terminal" && runtimes[hit.sessionId]?.status === "running";

  // Open or focus the session tab, then close the overlay. Agent sessions resume their conversation.
  const openSessionFromSearch = (hit: SessionSearchHit) => {
    // Look for the literal the index matched in the first hit, not the raw query.
    const q = (hit.matches[0]?.matched[0] ?? query).trim();
    openSession(hit.sessionId);
    close();
    // Best effort: jump to the match in a live terminal after opening the session.
    if (q && isLiveTerminal(hit)) {
      setTimeout(() => findNext(hit.sessionId, q), 80);
    }
  };

  const emptyHint = loading
    ? t("search.searching")
    : query.trim()
      ? t("search.noResults")
      : t("search.hint");

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
        {/* Header: query, project/collection filter, include-archived toggle, actual totals, and close button. */}
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
            <Icons.search size={16} />
          </span>
          <div className="box" style={{ display: "flex", alignItems: "center", gap: 6, flex: 1, minWidth: 0 }}>
            <input
              ref={inputRef}
              placeholder={
                activeProject
                  ? t("search.inLocationPlaceholder", projectDisplayName(t, activeProject))
                  : t("search.allPlaceholder")
              }
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  close();
                } else if (e.key === "ArrowDown") {
                  e.preventDefault();
                  nav.next();
                } else if (e.key === "ArrowUp") {
                  e.preventDefault();
                  nav.prev();
                } else if (e.key === "Enter") {
                  e.preventDefault();
                  if (e.shiftKey) nav.prev();
                  else nav.next();
                }
              }}
              style={{
                flex: 1,
                minWidth: 0,
                background: "transparent",
                border: "none",
                outline: "none",
                color: "var(--text-primary)",
                fontSize: 13,
              }}
            />
          </div>
          {locationOptions.length > 1 && (
            <Select
              size="sm"
              width={200}
              menuWidth={280}
              align="right"
              menuPortal
              searchable
              searchPlaceholder={t("newSession.locationSearch")}
              noMatchLabel={t("tree.noMatch")}
              ariaLabel={t("search.filterLocation")}
              title={selectedLocationLabel}
              value={activeProjectId}
              onChange={setProjectId}
              options={locationOptions}
            />
          )}
          <label
            title={t("search.includeArchivedHint")}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 5,
              flex: "0 0 auto",
              cursor: "pointer",
              userSelect: "none",
              fontSize: 12,
              color: includeArchived ? "var(--text-primary)" : "var(--text-muted)",
            }}
          >
            <input
              type="checkbox"
              checked={includeArchived}
              onChange={(e) => setIncludeArchived(e.target.checked)}
              style={{ cursor: "pointer", margin: 0 }}
            />
            {t("search.includeArchived")}
          </label>
          <span style={{ fontSize: 12, color: "var(--text-muted)", flex: "0 0 auto" }}>
            {loading
              ? t("search.searching")
              : query.trim() && results.length > 0
                ? `${t("search.summary", totalMatches, sessions)}${
                    locatable < totalMatches ? ` (${t("search.cappedNote", locatable, totalMatches)})` : ""
                  }`
                : ""}
          </span>
          <button className="icon-btn sm" title={t("common.close")} onClick={close}>
            <Icons.x size={14} />
          </button>
        </div>

        {/* Main area: two-pane search console. */}
        <ContentSearchControls search={search} />
        <SearchConsole
          nav={nav}
          results={results}
          sessionById={sessionById}
          projects={projects}
          groups={groups}
          liveSessions={liveSessions}
          onOpenSession={openSessionFromSearch}
          emptyHint={emptyHint}
        />
      </div>
    </Backdrop>
  );
}
