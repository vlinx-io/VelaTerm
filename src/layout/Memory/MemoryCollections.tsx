//! Knowledge-base Collections: archived sessions kept alongside knowledge entries. Both the right tree
//! and the main area arrange archived sessions as they sat in the session tree — project, group chain,
//! then parent and child sessions. The main area also searches their full content and reads one
//! conversation together with the entries generated from it.
//!
//! The archive overlay that used to host this functionality was removed; restore, export, delete and
//! organize actions all live here now.

import { Fragment, useEffect, useMemo, useState } from "react";
import { ContextMenu } from "../../components/ContextMenu";
import Icons from "../../components/Icons";
import Select from "../../components/Select";
import { canExportContext, exportSessionToFile } from "../../exportSession";
import { useT } from "../../i18n";
import { memoryCollections, memoryList, type MemoryCollectionProject, type MemoryCollectionSession } from "../../ipc/memory";
import { useTermStore } from "../../store/termStore";
import type { Session, SessionKind } from "../../types";
import { KnowledgeBackLink } from "../Notebook/KnowledgeBack";
import { kindLabel } from "../sessionMenuShared";
import { ContentSearchControls, SearchConsole, useContentSearch, useSearchNav } from "../GlobalSearch/searchKit";
import { KnowledgeTreeRow, knowledgeSelection } from "../Notebook/KnowledgeTreeRow";
import { SessionContentViewer } from "../sessionViewers/SessionContentViewer";
import { fmtArchivedAt, KindIcon, kindIconEl, locationOf } from "../sessionViewers/sessionMeta";
import { COLLECTIONS_CHANGED, COLLECTIONS_COLLAPSE, MemoryLink, memoryNavigate, memoryUrl, useMemoryLocation } from "./navigation";
import { LoadState, MemoryIcon, shortDate, useMemoryLoad } from "./shared";
import "./memory.css";

/** Window event that tells every collections surface to reload after a restore or permanent delete. */

function projectName(project: MemoryCollectionProject, fallback: string): string {
  return project.name || fallback;
}

/** One node of a project's archive tree: a group from the original container chain, or a session. */
export type CollectionNode =
  | { kind: "group"; key: string; name: string; children: CollectionNode[] }
  | { kind: "session"; key: string; session: MemoryCollectionSession; children: CollectionNode[] };

const groupKey = (id: string) => `g:${id}`;
const sessionKey = (id: string) => `s:${id}`;

/** Arrange one project's archived sessions as they sat in the session tree: roots under their group
 *  chain, descendants under their archived parent. Groups lead each level by name; sessions keep the
 *  backend's newest-archived-first order. */
export function collectionTree(sessions: MemoryCollectionSession[]): CollectionNode[] {
  const roots: CollectionNode[] = [];
  const groups = new Map<string, CollectionNode>();
  const nodes = new Map<string, CollectionNode>(sessions.map((session) => [session.id, { kind: "session", key: sessionKey(session.id), session, children: [] }]));
  for (const session of sessions) {
    const node = nodes.get(session.id)!;
    const parent = session.parentId ? nodes.get(session.parentId) : undefined;
    if (parent) { parent.children.push(node); continue; }
    let level = roots;
    // An older remote backend omits group IDs; its roots then sit directly under the project.
    (session.groupIds ?? []).forEach((id, index) => {
      let group = groups.get(id);
      if (!group) {
        group = { kind: "group", key: groupKey(id), name: session.groupPath[index] ?? "", children: [] };
        groups.set(id, group);
        level.push(group);
      }
      level = group.children;
    });
    level.push(node);
  }
  const rank = (node: CollectionNode) => (node.kind === "group" ? 0 : 1);
  const order = (level: CollectionNode[]) => {
    level.sort((a, b) => rank(a) - rank(b) || (a.kind === "group" && b.kind === "group" ? a.name.localeCompare(b.name) : 0));
    for (const node of level) order(node.children);
  };
  order(roots);
  return roots;
}

/** One visible row of the archive's tree table. */
export interface CollectionTreeRow {
  kind: "project" | "group" | "session";
  key: string;
  depth: number;
  name: string;
  project: MemoryCollectionProject;
  session?: MemoryCollectionSession;
  expandable: boolean;
  open: boolean;
  /** Sessions and knowledge entries at or below this row. */
  sessions: number;
  knowledge: number;
}

function keepMatching(nodes: CollectionNode[], matches: (session: MemoryCollectionSession) => boolean): CollectionNode[] {
  return nodes.flatMap((node) => {
    const children = keepMatching(node.children, matches);
    return children.length || (node.kind === "session" && matches(node.session)) ? [{ ...node, children }] : [];
  });
}

function orderSiblings(nodes: CollectionNode[], sort: string): CollectionNode[] {
  const at = (node: CollectionNode) => (node.kind === "session" ? node.session.archivedAt ?? 0 : 0);
  return [...nodes]
    .sort((a, b) => a.kind === "group" && b.kind === "group" ? a.name.localeCompare(b.name) : a.kind === "group" ? -1 : b.kind === "group" ? 1 : (at(a) - at(b)) * (sort === "oldest" ? 1 : -1))
    .map((node) => ({ ...node, children: orderSiblings(node.children, sort) }));
}

function totals(nodes: CollectionNode[]): { sessions: number; knowledge: number } {
  return nodes.reduce((sum, node) => {
    const below = totals(node.children);
    const own = node.kind === "session" ? { sessions: 1, knowledge: node.session.count } : { sessions: 0, knowledge: 0 };
    return { sessions: sum.sessions + own.sessions + below.sessions, knowledge: sum.knowledge + own.knowledge + below.knowledge };
  }, { sessions: 0, knowledge: 0 });
}

/** Flatten the archive into the rows the tree table shows: projects by name, then groups, then sessions in the
 *  chosen date order. A filter keeps matching sessions with what sits above them, and opens the way to them. */
export function collectionTreeRows(projects: MemoryCollectionProject[], options: {
  expanded: ReadonlySet<string>; sort: string; matches?: (session: MemoryCollectionSession) => boolean; openAll?: boolean; openProject?: string; fallbackName: string;
}): CollectionTreeRow[] {
  const rows: CollectionTreeRow[] = [];
  const walk = (project: MemoryCollectionProject, nodes: CollectionNode[], depth: number) => {
    for (const node of nodes) {
      const open = !!options.openAll || options.expanded.has(node.key);
      rows.push({
        kind: node.kind, key: node.key, depth, project, name: node.kind === "group" ? node.name : node.session.name,
        session: node.kind === "session" ? node.session : undefined, expandable: node.children.length > 0, open, ...totals([node]),
      });
      if (open) walk(project, node.children, depth + 1);
    }
  };
  for (const project of [...projects].sort((a, b) => projectName(a, options.fallbackName).localeCompare(projectName(b, options.fallbackName)))) {
    const tree = collectionTree(project.sessions);
    const nodes = orderSiblings(options.matches ? keepMatching(tree, options.matches) : tree, options.sort);
    if (!nodes.length) continue;
    const open = !!options.openAll || options.openProject === project.id || options.expanded.has(project.id);
    rows.push({ kind: "project", key: project.id, depth: 0, project, name: projectName(project, options.fallbackName), expandable: true, open, ...totals(nodes) });
    if (open) walk(project, nodes, 1);
  }
  return rows;
}

/** Tree rows opened in the main table, kept while the page is left to read a session and come back. */
const openTreeRows = new Set<string>();

/** Groups and parent sessions above one archived session, top-down, so a tree can expand the path
 *  to it and labels can show where it was stored. */
export function collectionLineage(byId: Map<string, MemoryCollectionSession>, id: string): { key: string; name: string; sessionId?: string }[] {
  const lineage: { key: string; name: string; sessionId?: string }[] = [];
  let current = byId.get(id);
  const seen = new Set<string>();
  while (current?.parentId && !seen.has(current.id)) {
    seen.add(current.id);
    const parent = byId.get(current.parentId);
    if (!parent) break;
    lineage.unshift({ key: sessionKey(parent.id), name: parent.name, sessionId: parent.id });
    current = parent;
  }
  if (current) lineage.unshift(...(current.groupIds ?? []).map((group, index) => ({ key: groupKey(group), name: current!.groupPath[index] ?? "" })));
  return lineage;
}

/** Where an archived session was stored, for labels: its project and the groups above it. */
export function collectionPlace(projectLabel: string, byId: Map<string, MemoryCollectionSession>, id: string): string {
  return [projectLabel, ...collectionLineage(byId, id).filter((node) => !node.sessionId).map((node) => node.name)].filter(Boolean).join(" / ");
}

/** Every archived session row by ID, across projects. */
function useCollectionRows(data: { projects: MemoryCollectionProject[] } | null) {
  return useMemo(() => new Map((data?.projects ?? []).flatMap((project) => project.sessions.map((session) => [session.id, session] as const))), [data]);
}

/** Right-panel tree: archived sessions under the project, groups and parent sessions they were archived from. */
export function MemoryCollectionsDirectory({ baseDepth = 0 }: { baseDepth?: number } = {}) {
  const t = useT();
  const location = useMemoryLocation();
  const params = new URLSearchParams(location);
  const [page, id = ""] = (params.get("memory") ?? "").split("/");
  const projectId = params.get("memoryCollectionProject") ?? "";
  const { data, error, reload } = useMemoryLoad(memoryCollections, []);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const open = (...keys: string[]) => setExpanded((old) => (keys.every((key) => old.has(key)) ? old : new Set([...old, ...keys])));
  const toggle = (key: string) =>
    setExpanded((old) => {
      const next = new Set(old);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  const rowById = useCollectionRows(data);
  const trees = useMemo(() => new Map((data?.projects ?? []).map((project) => [project.id, collectionTree(project.sessions)])), [data]);
  useEffect(() => {
    const project = projectId || data?.projects.find((p) => p.sessions.some((s) => s.id === id))?.id;
    if (project) open(project, ...(id ? collectionLineage(rowById, id).map((node) => node.key) : []));
  }, [data, projectId, id, rowById]);
  useEffect(() => {
    window.addEventListener(COLLECTIONS_CHANGED, reload);
    return () => window.removeEventListener(COLLECTIONS_CHANGED, reload);
  }, [reload]);
  useEffect(() => {
    const collapse = () => setExpanded(new Set());
    window.addEventListener(COLLECTIONS_COLLAPSE, collapse);
    return () => window.removeEventListener(COLLECTIONS_COLLAPSE, collapse);
  }, []);
  const sessionById = useMemo(() => new Map((data?.sessions ?? []).map((s) => [s.id, s])), [data]);
  const branches = (project: MemoryCollectionProject, nodes: CollectionNode[], depth: number) => <ul className="nb-tree-branches">
    {nodes.map((node) => {
      if (node.kind === "group") {
        return <KnowledgeTreeRow key={node.key} name={node.name} depth={depth} icon={<Icons.folder size={14} />}
          expanded={expanded.has(node.key)} onToggle={() => toggle(node.key)} onActivate={() => toggle(node.key)}>
          {branches(project, node.children, depth + 1)}
        </KnowledgeTreeRow>;
      }
      const session = node.session;
      const live = sessionById.get(session.id);
      const nested = node.children.length > 0;
      return <KnowledgeTreeRow key={node.key} prose name={session.name} route={`collection/${session.id}`}
        values={{ ...knowledgeSelection, memoryCollectionProject: project.id, memoryCollectionQuery: null }} depth={depth}
        icon={live ? <KindIcon session={live} size={14} /> : <Icons.bot size={14} />} knowledge={session.count}
        title={[projectName(project, t("memory.unknownProject")), ...collectionLineage(rowById, session.id).map((item) => item.name), session.name].filter(Boolean).join(" › ")}
        selected={page === "collection" && id === session.id}
        expanded={nested ? expanded.has(node.key) : undefined} onToggle={nested ? () => toggle(node.key) : undefined}>
        {nested && branches(project, node.children, depth + 1)}
      </KnowledgeTreeRow>;
    })}
  </ul>;
  return <ul className="nb-tree-branches memory-directory">
    {(!data || error) && <li><LoadState error={error} reload={reload} /></li>}
    {data && !data.projects.length && <li className="nb-tree-empty">{t("archive.empty1")}</li>}
    {data?.projects.map((project) => <KnowledgeTreeRow key={project.id} name={projectName(project, t("memory.unknownProject"))}
      route="collections" values={{ ...knowledgeSelection, memoryCollectionProject: project.id, memoryCollectionQuery: null }} depth={baseDepth}
      icon={<Icons.folder size={14} />} count={project.sessions.length} knowledge={project.count}
      selected={page === "collections" && projectId === project.id && !id}
      expanded={expanded.has(project.id)} onToggle={() => toggle(project.id)} onOpen={() => open(project.id)}>
      {branches(project, trees.get(project.id) ?? [], baseDepth + 1)}
    </KnowledgeTreeRow>)}
  </ul>;
}

/** Rows added each time the list is extended; the archive can hold thousands of sessions. */
const LIST_STEP = 100;

/** Main area: browse, archive-wide search, and one archived conversation with its knowledge entries. */
export function MemoryCollections({ sessionId }: { sessionId: string }) {
  const t = useT();
  const location = useMemoryLocation();
  const params = new URLSearchParams(location);
  const query = params.get("memoryCollectionQuery") ?? "";
  const projectId = params.get("memoryCollectionProject") ?? "";
  const agent = params.get("memoryCollectionAgent") ?? "";
  const knowledge = params.get("memoryCollectionKnowledge") ?? "";
  const sort = params.get("memoryCollectionSort") ?? "newest";
  const { data, error, reload } = useMemoryLoad(memoryCollections, []);
  const loadArchived = useTermStore((s) => s.loadArchived);
  const restoreSession = useTermStore((s) => s.restoreSession);
  const deleteNode = useTermStore((s) => s.deleteNode);
  const liveProjects = useTermStore((s) => s.projects);
  const groups = useTermStore((s) => s.groups);
  const liveSessions = useTermStore((s) => s.sessions);
  const rightCollapsed = useTermStore((s) => s.rightCollapsed);
  useEffect(() => { void loadArchived(); }, [loadArchived]);
  useEffect(() => {
    window.addEventListener(COLLECTIONS_CHANGED, reload);
    return () => window.removeEventListener(COLLECTIONS_CHANGED, reload);
  }, [reload]);
  const search = useContentSearch("archived", query);
  const nav = useSearchNav(search.results);
  const totalMatches=search.page?.totalMatches ?? 0;const matchedSessions=search.page?.totalSessions ?? 0;
  const sessionById = useMemo(() => new Map((data?.sessions ?? []).map((s) => [s.id, s])), [data]);
  const rowById = useCollectionRows(data);
  const view = params.get("memoryCollectionView") === "time" ? "time" : "tree";
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => new Set(openTreeRows));
  const toggleTreeRow = (key: string) => {
    if (openTreeRows.has(key)) openTreeRows.delete(key); else openTreeRows.add(key);
    setExpanded(new Set(openTreeRows));
  };
  const [limit, setLimit] = useState(LIST_STEP);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [rowMenu, setRowMenu] = useState<{ x: number; y: number; session: Session } | null>(null);
  const session = sessionId ? sessionById.get(sessionId) : undefined;
  const project = data?.projects.find((p) => p.sessions.some((s) => s.id === sessionId)) ?? data?.projects.find((p) => p.id === projectId);
  const row = project?.sessions.find((s) => s.id === sessionId);
  const lineage = sessionId ? collectionLineage(rowById, sessionId) : [];
  const projectLabel = project ? projectName(project, t("memory.unknownProject")) : "";
  // Tombstoned projects and groups are not in the live tree, so search results use the stored path.
  const storedPaths = useMemo(() => {
    const paths = new Map<string, string>();
    for (const item of data?.projects ?? []) {
      const name = projectName(item, t("memory.unknownProject"));
      for (const archived of item.sessions) paths.set(archived.id, [name, ...collectionLineage(rowById, archived.id).map((node) => node.name)].filter(Boolean).join(" \u203a "));
    }
    return paths;
  }, [data, rowById, t]);
  // The default tab follows the stored entry count; an explicit choice is carried in the URL.
  const storedTab = params.get("memoryCollectionTab");
  // A session with no entries is just its conversation, so it shows no tabs until it has been organized.
  const tab = !row?.count ? "conversation" : storedTab === "conversation" || storedTab === "entries" ? storedTab : "entries";
  const refresh = () => { reload(); void loadArchived(); };
  const changed = () => { window.dispatchEvent(new Event(COLLECTIONS_CHANGED)); void loadArchived(); };
  // Restoring or deleting a session takes its whole subtree, so the open session may go with an ancestor.
  const leaves = (id: string) => id === sessionId || lineage.some((node) => node.sessionId === id);
  const restore = async (id: string) => {
    await restoreSession(id);
    changed();
    if (leaves(id)) memoryNavigate(memoryUrl("collections", { memoryCollectionProject: project?.id ?? null }));
  };
  const remove = async (id: string) => {
    await deleteNode("session", id);
    await loadArchived();
    window.dispatchEvent(new Event(COLLECTIONS_CHANGED));
    if (leaves(id)) memoryNavigate(memoryUrl("collections", { memoryCollectionProject: project?.id ?? null }));
  };
  // A nested session is restored with its archived parent, never on its own below an archived parent.
  const actions = (target: Session) => <>
    {!rowById.get(target.id)?.parentId && <button type="button" className="kbx-ibtn" title={t("archive.restore")} aria-label={t("archive.restore")} onClick={() => void restore(target.id)}><Icons.restart size={14} aria-hidden="true" /></button>}
    <MemoryLink route={`compile/${target.id}`} className="kbx-ibtn" title={t("memory.add")} aria-label={t("memory.add")}><MemoryIcon size={14} /></MemoryLink>
    {canExportContext(target) && <button type="button" className="kbx-ibtn" title={t("archive.export")} aria-label={t("archive.export")} onClick={() => void exportSessionToFile(target)}><Icons.download size={14} aria-hidden="true" /></button>}
    <button type="button" className="kbx-ibtn" title={t("archive.deleteForever")} aria-label={t("archive.deleteForever")} style={{ color: "var(--status-error)" }} onClick={() => void remove(target.id)}><Icons.trash size={14} aria-hidden="true" /></button>
  </>;
  const flat = useMemo(() => (data?.projects ?? []).flatMap((item) => item.sessions.map((archived) => ({ project: item, archived }))), [data]);
  const agents = useMemo(() => [...new Set(flat.map(({ archived }) => archived.kind))], [flat]);
  const rows = useMemo(() => flat
    .filter(({ project: item, archived }) => (!projectId || item.id === projectId) && (!agent || archived.kind === agent) && (!knowledge || (knowledge === "added") === archived.count > 0))
    .sort((a, b) => ((a.archived.archivedAt ?? 0) - (b.archived.archivedAt ?? 0)) * (sort === "oldest" ? 1 : -1)), [flat, projectId, agent, knowledge, sort]);
  const organized = useMemo(() => rows.filter(({ archived }) => archived.count > 0).length, [rows]);
  const treeRows = useMemo(() => {
    const narrowed = !!agent || !!knowledge;
    return collectionTreeRows((data?.projects ?? []).filter((item) => !projectId || item.id === projectId), {
      expanded, sort, openAll: narrowed, openProject: projectId, fallbackName: t("memory.unknownProject"),
      matches: narrowed ? (archived) => (!agent || archived.kind === agent) && (!knowledge || (knowledge === "added") === archived.count > 0) : undefined,
    });
  }, [data, projectId, agent, knowledge, sort, expanded, t]);
  useEffect(() => setLimit(LIST_STEP), [projectId, agent, knowledge, sort, view]);
  const setFilter = (values: Record<string, string | null>) => memoryNavigate(memoryUrl("collections", values), true);
  if (sessionId) return <div className="kbx-page">
    <div className="kbx-bar">
      <KnowledgeBackLink className="kbx-ibtn" />
      <nav className="kbx-crumbs" aria-label={t("memory.hierarchy")}>
        <MemoryLink route="collections" values={{ memoryCollectionProject: null, memoryCollectionQuery: null, memoryCollectionTab: null }}>{t("memory.collections")}</MemoryLink>
        {project && <><Icons.chevR size={12} aria-hidden="true" /><MemoryLink className="cur" route="collections" values={{ memoryCollectionProject: project.id, memoryCollectionQuery: null, memoryCollectionTab: null }}>{projectLabel}</MemoryLink></>}
        {lineage.filter((node) => node.sessionId).map((node) => <Fragment key={node.key}><Icons.chevR size={12} aria-hidden="true" /><MemoryLink route={`collection/${node.sessionId}`} values={{ memoryCollectionProject: project?.id ?? null, memoryCollectionQuery: null, memoryCollectionTab: null }}>{node.name}</MemoryLink></Fragment>)}
      </nav>
      {session && row && <div className="kbx-acts">
        {!row.parentId && <button type="button" className="kbx-btn" title={t("archive.restore")} onClick={() => void restore(session.id)}><Icons.restart size={14} aria-hidden="true" />{t("archive.restoreShort")}</button>}
        <button type="button" className="kbx-ibtn" title={t("kb.moreActions")} aria-label={t("kb.moreActions")} onClick={(event) => { const rect = event.currentTarget.getBoundingClientRect(); setMenu({ x: rect.right - 240, y: rect.bottom + 4 }); }}><Icons.more size={16} aria-hidden="true" /></button>
        <MemoryLink className={"kbx-btn" + (row.count ? "" : " pri")} route={`compile/${session.id}`}><MemoryIcon size={14} />{t("memory.add")}</MemoryLink>
      </div>}
    </div>
    {menu && session && <ContextMenu x={menu.x} y={menu.y} onClose={() => setMenu(null)} items={[
      ...(canExportContext(session) ? [{ label: t("archive.export"), onClick: () => void exportSessionToFile(session) }] : []),
      { label: t("archive.deleteForever"), danger: true, onClick: () => void remove(session.id) },
    ]} />}
    {!data ? <LoadState error={error} reload={refresh} /> : (!session || !row) ? <p className="kbx-empty">{t("memory.notFound")}</p> : <>
      <div className={"kbx-head" + (row.count ? "" : " closed")}><div className="t">
        <div className="t1">{kindIconEl(row.kind as SessionKind, 16)}<h1 className="kbx-p">{session.name}</h1></div>
        {rightCollapsed && <div className="kbx-meta"><span>{kindLabel(row.kind as SessionKind)}</span><span>{fmtArchivedAt(row.archivedAt)}</span></div>}
      </div></div>
      {row.count > 0 && <div className="kbx-tabs" role="tablist">
        <MemoryLink role="tab" aria-selected={tab === "conversation"} className={tab === "conversation" ? "on" : ""}
          route={`collection/${session.id}`} values={{ memoryCollectionProject: project?.id ?? null, memoryCollectionTab: "conversation", memoryCollectionQuery: null }}>{t("memory.collectionConversation")}</MemoryLink>
        <MemoryLink role="tab" aria-selected={tab === "entries"} className={tab === "entries" ? "on" : ""}
          route={`collection/${session.id}`} values={{ memoryCollectionProject: project?.id ?? null, memoryCollectionTab: "entries", memoryCollectionQuery: null }}>{t("memory.entries")}<i>{row.count}</i></MemoryLink>
      </div>}
      {tab === "conversation"
        ? <div className="kbx-viewer memory-collection-viewer"><SessionContentViewer key={session.id} session={session} /></div>
        : <CollectionEntries sessionId={session.id} />}
    </>}
  </div>;
  return <div className="kbx-page kbx-narrow">
    <div className="kbx-bar top">
      <div className="kbx-crumbs"><h1 className="kbx-bar-title">{project ? projectLabel : t("memory.collections")}</h1>{data && <span>{[project ? t("archive.sessionsCount", rows.length.toLocaleString()) : t("archive.sessionsInProjects", rows.length.toLocaleString(), String(data.projects.length)), organized > 0 && t("kb.inKnowledgeBase", organized.toLocaleString())].filter(Boolean).join(" · ")}</span>}</div>
      <div className="kbx-acts"><button type="button" className="kbx-ibtn" title={t("common.refresh")} aria-label={t("common.refresh")} onClick={refresh}><Icons.restart size={14} aria-hidden="true" /></button></div>
    </div>
    <div className="kbx-filter">
      <label className="kbx-input">
        <Icons.search size={12} aria-hidden="true" />
        <input aria-label={t("archive.searchPlaceholder")} placeholder={t("archive.searchPlaceholder")} value={query}
          onChange={(e) => memoryNavigate(memoryUrl("collections", { memoryCollectionProject: null, memoryCollectionQuery: e.target.value || null }), true)} />
      </label>
      {query.trim() ? <span className="memory-browser-count">{search.loading ? t("search.searching") : t("search.summary", totalMatches, matchedSessions)}</span> : <>
        <Select size="sm" width="auto" menuWidth={220} value={projectId} ariaLabel={t("archive.colProject")} onChange={(value) => setFilter({ memoryCollectionProject: value || null })} options={[{ value: "", label: t("archive.allProjects") }, ...(data?.projects ?? []).map((item) => ({ value: item.id, label: projectName(item, t("memory.unknownProject")) }))]} />
        <Select size="sm" width="auto" menuWidth={180} value={agent} ariaLabel={t("archive.colAgent")} onChange={(value) => setFilter({ memoryCollectionAgent: value || null })} options={[{ value: "", label: t("archive.allAgents") }, ...agents.map((kind) => ({ value: kind, label: kindLabel(kind as SessionKind) }))]} />
        <Select size="sm" width="auto" menuWidth={220} value={knowledge} ariaLabel={t("archive.colKnowledge")} onChange={(value) => setFilter({ memoryCollectionKnowledge: value || null })} options={[{ value: "", label: t("archive.anyKnowledge") }, { value: "added", label: t("archive.withKnowledge") }, { value: "none", label: t("archive.withoutKnowledge") }]} />
        <span className="g" />
        <Select size="sm" width="auto" menuWidth={140} value={view} ariaLabel={t("archive.view")} onChange={(value) => setFilter({ memoryCollectionView: value === "tree" ? null : value })} options={[{ value: "tree", label: t("archive.viewTree") }, { value: "time", label: t("archive.viewTime") }]} />
        <Select size="sm" width="auto" menuWidth={180} value={sort} ariaLabel={t("archive.colArchived")} onChange={(value) => setFilter({ memoryCollectionSort: value === "newest" ? null : value })} options={[{ value: "newest", label: t("archive.newest") }, { value: "oldest", label: t("archive.oldest") }]} />
      </>}
    </div>
    {!data ? <LoadState error={error} reload={refresh} /> : query.trim() ? <div className="kbx-viewer">
      <ContentSearchControls search={search} /><SearchConsole nav={nav} results={search.results} sessionById={sessionById} projects={liveProjects} groups={groups} liveSessions={liveSessions}
        sessionLocation={(s) => storedPaths.get(s.id) ?? locationOf(s, liveProjects, groups, liveSessions)}
        renderGroupActions={(_hit, target) => (target ? actions(target) : null)}
        emptyHint={search.error ?? (search.loading ? t("search.searching") : t("search.noResults"))} />
    </div> : <div className="kbx-list">
      {!flat.length ? <p className="kbx-empty">{t("archive.empty1")}<br />{t("archive.empty2")}</p> : view === "tree" ? <div>
        <div className="kbx-tr tree hd"><span>{t("archive.colSession")}</span><span>{t("archive.colAgent")}</span><span>{t("archive.colKnowledge")}</span><span className="r">{t("archive.colArchived")}</span></div>
        {treeRows.slice(0, limit).map((row) => {
          const knowledgeCell = row.knowledge > 0 ? <span className="kbx-count"><Icons.layers size={14} aria-hidden="true" />{row.knowledge}</span> : <span />;
          const indent = { paddingLeft: row.depth * 18 };
          const chevron = row.open ? <Icons.chevD size={12} aria-hidden="true" /> : <Icons.chevR size={12} aria-hidden="true" />;
          if (!row.session) return <button type="button" key={row.key} className="kbx-tr tree node" aria-expanded={row.open} onClick={() => toggleTreeRow(row.key)}>
            <span className="kbx-tree-name" style={indent}>{chevron}<Icons.folder size={14} aria-hidden="true" /><span className="s">{row.name}</span><small>{row.sessions.toLocaleString()}</small></span>
            <span />{knowledgeCell}<span />
          </button>;
          const archived = row.session;
          return <div key={row.key} className="kbx-tr tree" onContextMenu={(event) => { const target = sessionById.get(archived.id); if (!target) return; event.preventDefault(); setRowMenu({ x: event.clientX, y: event.clientY, session: target }); }}>
            <span className="kbx-tree-name" style={indent}>
              {row.expandable ? <button type="button" className="kbx-tree-toggle" aria-expanded={row.open} aria-label={archived.name} onClick={() => toggleTreeRow(row.key)}>{chevron}</button> : <span className="kbx-tree-toggle" />}
              <MemoryLink className="s kbx-tree-link" title={`${archived.name}\n${fmtArchivedAt(archived.archivedAt)}`} route={`collection/${archived.id}`} values={{ memoryCollectionProject: row.project.id, memoryCollectionTab: null }}>{archived.name}</MemoryLink>
            </span>
            <span className="c">{kindIconEl(archived.kind as SessionKind, 14)}{kindLabel(archived.kind as SessionKind)}</span>
            {knowledgeCell}
            <span className="c r">{archived.archivedAt ? shortDate(archived.archivedAt * 1000) : ""}</span>
          </div>;
        })}
      </div> : <div>
        <div className="kbx-tr hd"><span>{t("archive.colSession")}</span><span>{t("archive.colProject")}</span><span>{t("archive.colAgent")}</span><span>{t("archive.colKnowledge")}</span><span className="r">{t("archive.colArchived")}</span></div>
        {rows.slice(0, limit).map(({ project: item, archived }, index, shown) => {
          const date = archived.archivedAt ? shortDate(archived.archivedAt * 1000) : "";
          // Rows are in date order, so a day's date appears once, on its first row.
          const sameDay = index > 0 && shown[index - 1].archived.archivedAt && shortDate(shown[index - 1].archived.archivedAt! * 1000) === date;
          const place = collectionPlace(projectName(item, t("memory.unknownProject")), rowById, archived.id);
          return <MemoryLink key={archived.id} className="kbx-tr" title={`${archived.name}\n${fmtArchivedAt(archived.archivedAt)}`}
          onContextMenu={(event) => { const target = sessionById.get(archived.id); if (!target) return; event.preventDefault(); setRowMenu({ x: event.clientX, y: event.clientY, session: target }); }} route={`collection/${archived.id}`} values={{ memoryCollectionProject: item.id, memoryCollectionTab: null }}>
          <span className="s">{archived.name}</span>
          <span className="c" title={place}>{place}</span>
          <span className="c">{kindIconEl(archived.kind as SessionKind, 14)}{kindLabel(archived.kind as SessionKind)}</span>
          {archived.count > 0 ? <span className="kbx-count"><Icons.layers size={14} aria-hidden="true" />{archived.count}</span> : <span />}
          <span className="c r">{sameDay ? "" : date}</span>
        </MemoryLink>;
        })}
      </div>}
      {rowMenu && <ContextMenu x={rowMenu.x} y={rowMenu.y} onClose={() => setRowMenu(null)} items={[
        // A nested session is restored with its archived parent, never on its own.
        ...(!rowById.get(rowMenu.session.id)?.parentId ? [{ label: t("archive.restore"), onClick: () => void restore(rowMenu.session.id) }] : []),
        { label: t("memory.add"), onClick: () => memoryNavigate(memoryUrl(`compile/${rowMenu.session.id}`)) },
        ...(canExportContext(rowMenu.session) ? [{ label: t("archive.export"), onClick: () => void exportSessionToFile(rowMenu.session) }] : []),
        { label: t("archive.deleteForever"), danger: true, onClick: () => void remove(rowMenu.session.id) },
      ]} />}
      {(view === "tree" ? treeRows.length : rows.length) > limit && <div className="kbx-empty"><button type="button" className="kbx-btn" onClick={() => setLimit((value) => value + LIST_STEP)}>{t("nb.loadMore")}</button></div>}
    </div>}
  </div>;
}

/** Knowledge entries generated from one archived session, with the organize entry when empty. */
function CollectionEntries({ sessionId }: { sessionId: string }) {
  const t = useT();
  const { data, error, reload } = useMemoryLoad(() => memoryList({ query: "", tag: "", sort: "updated", page: 0, sessionId }), [sessionId]);
  if (!data) return <LoadState error={error} reload={reload} />;
  if (!data.entries.length) return <p className="kbx-empty">{t("memory.collectionEmptyEntries")} <MemoryLink route={`compile/${sessionId}`}>{t("memory.add")}</MemoryLink></p>;
  return <div className="kbx-list full memory-collection-entries">
    {data.entries.map((entry) => <MemoryLink key={entry.id} className="kbx-row" route={`entry/${entry.id}`} values={{ memoryProject: null, memorySession: sessionId, memoryCollectionTab: null, memoryCollectionProject: null }}>
      <div className="l1"><Icons.diamond size={14} className="kbx-ic-kb" aria-hidden="true" /><span className="tt">{entry.title}</span><span className="w">{shortDate(entry.updatedAt)}</span><Icons.chevR size={12} aria-hidden="true" /></div>
      {entry.summary && <div className="sn">{entry.summary}</div>}
    </MemoryLink>)}
  </div>;
}
