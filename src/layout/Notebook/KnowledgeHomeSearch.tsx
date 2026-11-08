//! Home-page lookup across both knowledge sources: session knowledge entries and local notes. The
//! home page has no single backend query for that pair, so one debounced handler runs both searches
//! in parallel and renders them as one keyboard-navigable result list grouped by source.

import { useEffect, useRef, useState, type ReactNode } from "react";
import Icons from "../../components/Icons";
import { useT } from "../../i18n";
import { memoryList, type MemoryList, type MemorySummary } from "../../ipc/memory";
import { notebookSearch, type NotebookSearchHit, type NotebookSearchResult } from "../../ipc/notebook";
import { highlightMatches } from "../sessionViewers/highlight";
import { memoryNavigate, memoryUrl } from "../Memory/navigation";
import { KnowledgeNoteRow, searchTerms, useSearchKeys } from "./KnowledgeSearch";
import { notebookError, openVaultFile } from "./shared";
import "./notebook.css";

/** Hits requested per source; the shown counts stay `total`, which counts every match. */
const SEARCH_LIMIT = 50;
/** Typing pause before a query runs, matching the other knowledge searches. */
const SEARCH_DELAY = 250;
const isMac = /Mac/.test(navigator.platform);

interface HomeSearch {
  notes: NotebookSearchResult | null;
  entries: MemoryList | null;
  error: string;
}

/** Debounced lookup across both sources. The previous page stays on screen while the next runs. */
export function useHomeSearch(query: string): HomeSearch {
  const [state, setState] = useState<HomeSearch>({ notes: null, entries: null, error: "" });
  useEffect(() => {
    const text = query.trim();
    if (!text) {
      setState({ notes: null, entries: null, error: "" });
      return;
    }
    let alive = true;
    setState((old) => ({ notes: old.notes, entries: old.entries, error: "" }));
    const timer = setTimeout(() => {
      Promise.allSettled([
        notebookSearch(text, "", SEARCH_LIMIT),
        memoryList({ query: text, tag: "", sort: "updated", page: 0 }),
      ]).then(([notes, entries]) => {
        if (!alive) return;
        setState({
          notes: notes.status === "fulfilled" ? notes.value : null,
          entries: entries.status === "fulfilled" ? entries.value : null,
          error: notes.status === "rejected" ? notebookError(notes.reason) : "",
        });
      });
    }, SEARCH_DELAY);
    return () => { alive = false; clearTimeout(timer); };
  }, [query]);
  return state;
}

type Row = { kind: "entry"; id: string } | { kind: "note"; note: NotebookSearchHit };

/** The search field plus its grouped results; the home sections show through while the query is empty. */
export function KnowledgeHomeSearch({ children }: { children: ReactNode }) {
  const t = useT();
  const [query, setQuery] = useState("");
  const lookup = useHomeSearch(query);
  const field = useRef<HTMLInputElement>(null);
  // The same shortcut that moves to the search box on a knowledge base page.
  useEffect(() => {
    const focus = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.shiftKey && event.key.toLowerCase() === "f") {
        event.preventDefault();
        event.stopImmediatePropagation();
        field.current?.focus();
      }
    };
    window.addEventListener("keydown", focus, true);
    return () => window.removeEventListener("keydown", focus, true);
  }, []);
  const searching = query.trim().length > 0;
  const entries = lookup.entries?.entries ?? [];
  const notes = lookup.notes?.entries ?? [];
  // Group path for one entry, resolved from the same hierarchy the memory browser uses.
  const group = (entry: MemorySummary) => {
    const id = entry.sessionId || "__manual__";
    const project = lookup.entries?.projects.find((item) => item.sessions.some((session) => session.id === id));
    const session = project?.sessions.find((item) => item.id === id);
    if (!project || !session) return "";
    const label = (item: { name: string; kind: string }) => item.name || t(item.kind === "manual" ? "memory.manualGroup" : item.kind === "legacy" ? "memory.legacyGroup" : "memory.unknownProject");
    return label(project) === label(session) ? label(session) : `${label(project)} / ${label(session)}`;
  };
  const rows: Row[] = [
    ...entries.map((entry) => ({ kind: "entry" as const, id: entry.id })),
    ...notes.map((note) => ({ kind: "note" as const, note })),
  ];
  const open = (index: number) => {
    const row = rows[index];
    if (!row) return;
    if (row.kind === "note") openVaultFile(row.note.absolutePath);
    else memoryNavigate(memoryUrl(`entry/${row.id}`));
  };
  const keys = useSearchKeys(rows.length, open, () => setQuery(""));
  const total = (lookup.notes?.total ?? 0) + (lookup.entries?.total ?? 0);
  const loaded = !!lookup.notes || !!lookup.entries;
  return <>
    <div className="kbx-search">
      <Icons.search size={16} aria-hidden="true" />
      <input ref={field} value={query} aria-label={t("nb.homeSearch")} placeholder={t("nb.homeSearch")} onKeyDown={keys.onKeyDown} onChange={(event) => setQuery(event.target.value)} />
      {searching ? <strong className="k" role="status">{loaded ? t("nb.searchCount", String(total)) : t("common.loading")}</strong> : <span className="k" aria-hidden="true">{isMac ? "\u21e7\u2318F" : "Ctrl+Shift+F"}</span>}
    </div>
    {!searching ? children : <div className="nb-home-results">
      {lookup.error && <p className="nb-message" role="alert">{lookup.error}</p>}
      {(lookup.notes?.fuzzy || lookup.entries?.fuzzy) && total > 0 && <p className="nb-search-hint" role="status">{t("nb.searchFuzzy")}</p>}
      {loaded && !total && <p className="nb-search-hint" role="status">{t("nb.searchEmptyAll")}</p>}
      {entries.length > 0 && <section className="nb-search-group">
        <h3><Icons.layers size={13} aria-hidden="true" />{t("memory.globalMemory")}<span>{lookup.entries?.total}</span></h3>
        <div className="nb-search-results" role="listbox" aria-label={t("memory.globalMemory")}>
          {entries.map((entry, index) => {
            const label = group(entry);
            const terms = searchTerms(query, entry.matched);
            return <div key={entry.id} role="option" aria-selected={keys.active === index}
              className={"nb-search-row" + (keys.active === index ? " active" : "")}
              onMouseEnter={() => keys.setActive(index)} onClick={() => open(index)}>
              <Icons.layers size={16} aria-hidden="true" />
              <div>
                <strong>{highlightMatches(entry.title, terms)}</strong>
                {label && <small>{label}</small>}
                {(entry.snippet || entry.summary) && <p>{highlightMatches(entry.snippet || entry.summary, terms)}</p>}
                <span className="nb-search-meta">{entry.line ? `${t("nb.searchLine", String(entry.line))} · ` : ""}{entry.tags.join(" · ")}</span>
                {entry.related && entry.related.length > 0 && <span className="nb-search-related">
                  {t("memory.related")}
                  {entry.related.map((related) => <button key={related.id} type="button" onClick={(event) => { event.stopPropagation(); memoryNavigate(memoryUrl(`entry/${related.id}`)); }}>{related.title}</button>)}
                </span>}
              </div>
            </div>;
          })}
        </div>
      </section>}
      {notes.length > 0 && <section className="nb-search-group">
        <h3><Icons.folder size={13} aria-hidden="true" />{t("nb.localVaults")}<span>{lookup.notes?.total}</span></h3>
        <div className="nb-search-results" role="listbox" aria-label={t("nb.localVaults")}>
          {notes.map((note, index) => {
            const absolute = entries.length + index;
            return <div key={`${note.vaultId}:${note.path}`} role="option" aria-selected={keys.active === absolute}
              className={"nb-search-row" + (keys.active === absolute ? " active" : "")}
              onMouseEnter={() => keys.setActive(absolute)} onClick={() => open(absolute)}>
              <KnowledgeNoteRow hit={note} query={query} onOpenRelated={(related) => openVaultFile(related.absolutePath)} />
            </div>;
          })}
        </div>
      </section>}
    </div>}
  </>;
}
