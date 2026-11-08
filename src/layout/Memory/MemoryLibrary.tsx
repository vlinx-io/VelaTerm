import { Fragment, useEffect, useState, type ReactNode } from "react";
import Icons from "../../components/Icons";
import Select from "../../components/Select";
import { useT } from "../../i18n";
import { memoryCollections, memoryList, type MemorySummary } from "../../ipc/memory";
import { highlightMatches } from "../sessionViewers/highlight";
import { KnowledgeBackLink } from "../Notebook/KnowledgeBack";
import { MemoryLink, memoryNavigate, memoryUrl, useMemoryLocation } from "./navigation";
import { LoadState, shortDate, useMemoryLoad } from "./shared";

export function MemoryLibrary({ selected, children }: { selected: string; children?: ReactNode }) {
  const t = useT(); const location = useMemoryLocation(); const params = new URLSearchParams(location);
  const query = params.get("memoryQuery") ?? ""; const tag = params.get("memoryTag") ?? "";
  const sort = params.get("memorySort") ?? "updated"; const page = Math.max(0, Number(params.get("memoryPage")) || 0);
  const projectId = params.get("memoryProject"); const sessionId = params.get("memorySession");
  const [input, setInput] = useState(query);
  const { data, error, reload: reloadEntries } = useMemoryLoad(() => memoryList({ query, tag, sort, page, ...(sessionId !== null ? { sessionId } : {}), ...(projectId !== null ? { projectId } : {}), selectedId: selected }), [query, tag, sort, page, selected, projectId, sessionId]);
  // Breadcrumbs describe the current folder even when its search has no matches.
  const hierarchy = useMemoryLoad(() => projectId || sessionId || selected
    ? memoryList({ query: "", tag: "", sort: "title", page: 0, selectedId: selected })
    : Promise.resolve(null), [projectId, sessionId, selected]);
  const directory = hierarchy.data ?? data;
  const archive = useMemoryLoad(memoryCollections, []);
  const reload = () => { reloadEntries(); hierarchy.reload(); };
  const activeSession = directory?.selectedSessionId ?? sessionId;
  const project = directory?.projects?.find((p) => p.sessions.some((s) => s.id === activeSession)) ?? directory?.projects?.find((p) => p.id === projectId);
  const activeProject = project?.id ?? projectId;
  const session = project?.sessions.find((s) => s.id === activeSession);
  const scope = { memoryProject: activeProject, memorySession: activeSession };
  const groupName = (group: { name: string; kind: string }) => group.name || t(group.kind === "manual" ? "memory.manualGroup" : group.kind === "legacy" ? "memory.legacyGroup" : "memory.unknownProject");
  // Filters keep the open entry: the list narrows beside it instead of closing the reader.
  const here = selected ? `entry/${selected}` : "library";
  useEffect(() => setInput(query), [query]);
  useEffect(() => {
    if (input === query) return;
    const timer = setTimeout(() => memoryNavigate(memoryUrl(here, { memoryQuery: input, memoryPage: null, memoryProject: activeProject, memorySession: activeSession }), true), 250);
    return () => clearTimeout(timer);
  }, [input, query, location, activeProject, activeSession, here]);
  useEffect(() => {
    window.addEventListener("memory:saved", reload);
    return () => window.removeEventListener("memory:saved", reload);
  }, [reload]);
  const search = <label className="kbx-input">
    <Icons.search size={12} aria-hidden="true" />
    <input aria-label={t("kb.searchEntries")} placeholder={t("kb.searchEntries")} value={input} onChange={(e) => setInput(e.target.value)} />
  </label>;
  const tagSelect = <Select size="sm" width="auto" menuWidth={180} value={tag} title={tag || t("memory.allTags")} ariaLabel={t("nb.tags")} onChange={(value) => memoryNavigate(memoryUrl(here, { ...scope, memoryTag: value, memoryPage: null }))} options={[{ value: "", label: t("memory.allTags") }, ...(tag && !data?.tags.includes(tag) ? [{ value: tag, label: tag }] : []), ...(data?.tags ?? []).map((value) => ({ value, label: value }))]} />;
  const sortSelect = <Select size="sm" width="auto" menuWidth={180} value={sort} ariaLabel={t("memory.updated")} onChange={(value) => memoryNavigate(memoryUrl(here, { ...scope, memorySort: value, memoryPage: null }))} options={[{ value: "updated", label: t("memory.updated") }, { value: "title", label: t("memory.titleSort") }]} />;
  const row = (entry: MemorySummary) => {
    const owner = data?.projects?.find((p) => p.sessions.some((s) => s.id === (entry.sessionId || "__manual__")));
    const origin = owner?.sessions.find((s) => s.id === (entry.sessionId || "__manual__"));
    const text = query && entry.snippet
      ? highlightMatches(entry.snippet, entry.matched?.length ? entry.matched : query.trim().split(/\s+/).filter(Boolean))
      : entry.summary;
    const link = <MemoryLink key={entry.id} route={`entry/${entry.id}`} values={{ memoryProject: owner?.id ?? activeProject, memorySession: origin?.id ?? activeSession }} className={"kbx-row" + (entry.id === selected ? " sel" : "")} aria-current={entry.id === selected ? "page" : undefined}>
      <div className="l1"><span className="tt two">{entry.title}</span><span className="w">{shortDate(entry.updatedAt)}</span></div>
      {text && <div className="sn">{text}</div>}
    </MemoryLink>;
    // Search results also point at neighbouring entries; links cannot nest, so they sit below the row.
    if (!query || !entry.related?.length) return link;
    return <Fragment key={entry.id}>{link}<div className="kbx-related"><span>{t("memory.related")}</span>{entry.related.map((related) => <MemoryLink key={related.id} route={`entry/${related.id}`} values={scope}>{related.title}</MemoryLink>)}</div></Fragment>;
  };
  const rows = !data ? <LoadState error={error} reload={reload} /> : !data.entries.length ? <p className="kbx-empty">{t("memory.empty")}</p> : data.entries.map(row);
  const pagination = data && data.total > data.pageSize && <footer className="memory-pagination">
    {page > 0 ? <MemoryLink route={here} values={{ ...scope, memoryPage: page - 1 }}>{t("common.prev")}</MemoryLink> : <span />}
    <span>{page + 1} / {Math.ceil(data.total / data.pageSize)}</span>
    {(page + 1) * data.pageSize < data.total && <MemoryLink route={here} values={{ ...scope, memoryPage: page + 1 }}>{t("common.next")}</MemoryLink>}
  </footer>;
  const archivedSession = session && archive.data?.projects.some((item) => item.sessions.some((row) => row.id === session.id));
  return <main className={"kbx-page" + (selected ? "" : " kbx-narrow")}>
    <div className="kbx-bar">
      <KnowledgeBackLink className="kbx-ibtn" />
      <nav className="kbx-crumbs" aria-label={t("memory.hierarchy")}>
        <MemoryLink route="library" values={{ memoryProject: null, memorySession: null, memoryPage: null }} className={project ? "" : "cur"}>{t("memory.globalMemory")}</MemoryLink>
        {project && <><Icons.chevR size={12} aria-hidden="true" /><MemoryLink route="library" values={{ memoryProject: project.id, memorySession: null, memoryPage: null }} className={session ? "" : "cur"}>{groupName(project)}</MemoryLink></>}
        {session && <><Icons.chevR size={12} aria-hidden="true" /><MemoryLink route="library" values={{ ...scope, memoryPage: null }} className="cur">{groupName(session)}</MemoryLink></>}
      </nav>
      <div className="kbx-acts">
        {archivedSession && <MemoryLink className="kbx-btn" route={`collection/${session.id}`} values={{ memoryProject: null, memorySession: null, memoryCollectionTab: "conversation" }}><Icons.archive size={14} aria-hidden="true" />{t("kb.openArchived")}</MemoryLink>}
        <MemoryLink className="kbx-btn" route="new"><Icons.plus size={14} aria-hidden="true" />{t("memory.new")}</MemoryLink>
      </div>
    </div>
    {!selected && <div className="kbx-head"><div className="t">
      <h1 className={session ? "kbx-p" : ""}>{session ? groupName(session) : project ? groupName(project) : t("memory.globalMemory")}</h1>
      <div className="kbx-meta">{data && <span>{t("kb.entriesCount", String(data.total))}</span>}</div>
    </div></div>}
    {!selected && <div className="kbx-filter">{search}{tagSelect}{sortSelect}<span className="g" /><button type="button" className="kbx-ibtn" aria-label={t("common.refresh")} title={t("common.refresh")} onClick={reload}><Icons.restart size={14} aria-hidden="true" /></button></div>}
    {selected ? <div className="kbx-read">{children}{error && <LoadState error={error} reload={reload} />}</div> : <div className="kbx-list full kbx-border-top">
      {data?.fuzzy && <p className="memory-muted">{t("nb.searchFuzzy")}</p>}
      {rows}{pagination}
    </div>}
  </main>;
}
