import { useEffect, useState } from "react";
import Icons from "../../components/Icons";
import { useT } from "../../i18n";
import { memoryCollections, memoryGet, memoryList, type MemoryEntry, type MemorySummary } from "../../ipc/memory";
import type { SessionKind } from "../../types";
import { kindIconEl } from "../sessionViewers/sessionMeta";
import { KnowledgeTreeRow, knowledgeSelection, treeKey, useKnowledgeTree } from "../Notebook/KnowledgeTreeRow";
import { useNotebookLoad } from "../Notebook/shared";
import { useMemoryLocation } from "./navigation";
import { LoadState, memoryError } from "./shared";
import "./memory.css";

/** Global memory is a tree: projects, then sessions, then the entries each session produced. */
export function MemoryDirectory() {
  const t = useT();
  const search = useMemoryLocation();
  const actions = useKnowledgeTree();
  const params = new URLSearchParams(search);
  const [page, id] = (params.get("memory") ?? "").split('/');
  const active = !!page && !["notebook", "notebooks"].includes(page);
  const selected = ["entry", "history", "edit"].includes(page) ? id : "";
  const projectId = active ? params.get("memoryProject") : null;
  const sessionId = active ? params.get("memorySession") : null;
  const { data, error, reload } = useNotebookLoad(() => memoryList({ query: "", tag: "", sort: "title", page: 0, selectedId: selected }), [page, selected, actions?.revision ?? 0], memoryError);
  const activeSession = active ? data?.selectedSessionId ?? sessionId : null;
  const project = data?.projects.find(p => p.sessions.some(s => s.id === activeSession));
  const activeProject = project?.id ?? projectId;
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const open = (key: string) => setExpanded(old => old.has(key) ? old : new Set(old).add(key));
  const toggle = (key: string) => setExpanded(old => { const next = new Set(old); if (next.has(key)) next.delete(key); else next.add(key); return next; });
  useEffect(() => {
    if (!active) return;
    setExpanded(old => {
      const next = new Set(old);
      if (activeProject) next.add(`project:${activeProject}`);
      if (activeSession) next.add(`session:${activeSession}`);
      return next;
    });
  }, [active, activeProject, activeSession]);
  useEffect(() => {
    const update = () => { if (!document.hidden) reload(); };
    window.addEventListener("focus", update);
    return () => window.removeEventListener("focus", update);
  }, [reload]);
  const [agentOf, setAgentOf] = useState<Map<string, string>>(new Map());
  useEffect(() => {
    let alive = true;
    // Icons only; the tree stays usable with the neutral icon when this fails.
    void (async () => {
      try {
        const all = await memoryCollections();
        if (alive) setAgentOf(new Map(all.projects.flatMap((project) => project.sessions.map((item) => [item.id, item.kind] as const))));
      } catch { /* keep the neutral icon */ }
    })();
    return () => { alive = false; };
  }, []);
  const groupName = (group: { name: string; kind: string }) => group.name || t(group.kind === "manual" ? "memory.manualGroup" : group.kind === "legacy" ? "memory.legacyGroup" : "memory.unknownProject");
  return <ul className="nb-tree-branches memory-directory">
    {(!data || error) && <li><LoadState error={error} reload={reload} /></li>}
    {data?.projects.map(item => <KnowledgeTreeRow key={item.id} name={groupName(item)} route="library" depth={0} icon={<Icons.folder size={14} />} count={item.count}
      target={{ key: treeKey.memoryProject(item.id), kind: "memoryProject", id: item.id, name: groupName(item), count: item.count, special: item.kind !== "project" }}
      values={{ ...knowledgeSelection, memoryProject: item.id }} selected={active && activeProject === item.id && !activeSession && page === "library"}
      expanded={expanded.has(`project:${item.id}`)} onToggle={() => toggle(`project:${item.id}`)} onOpen={() => open(`project:${item.id}`)}>
      <ul className="nb-tree-branches">
        {item.sessions.map(session => <KnowledgeTreeRow key={session.id} name={groupName(session)} route="library" depth={1} prose icon={agentOf.has(session.id) ? kindIconEl(agentOf.get(session.id) as SessionKind, 14) : <Icons.bot size={14} />} count={session.count}
          target={{ key: treeKey.memorySession(session.id), kind: "memorySession", id: session.id, name: groupName(session), count: session.count, special: session.kind !== "session" }}
          values={{ ...knowledgeSelection, memoryProject: item.id, memorySession: session.id }} selected={active && activeSession === session.id && page === "library"}
          expanded={expanded.has(`session:${session.id}`)} onToggle={() => toggle(`session:${session.id}`)} onOpen={() => open(`session:${session.id}`)}>
          <SessionEntries projectId={item.id} sessionId={session.id} selected={activeSession === session.id ? selected : ""} revision={activeSession === session.id ? page : ""} />
        </KnowledgeTreeRow>)}
      </ul>
    </KnowledgeTreeRow>)}
    {data && !data.projects.length && <li className="nb-tree-empty">{t("memory.empty")}</li>}
  </ul>;
}

function SessionEntries({ projectId, sessionId, selected, revision }: { projectId: string; sessionId: string; selected: string; revision: string }) {
  const t = useT();
  const actions = useKnowledgeTree();
  const { data, error, reload } = useNotebookLoad(() => memoryList({ query: "", tag: "", sort: "title", page: 0, projectId, sessionId }), [projectId, sessionId, revision, actions?.revision ?? 0], memoryError);
  const [extra, setExtra] = useState<MemorySummary[]>([]);
  const [page, setPage] = useState(0);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState("");
  const [selectedEntry, setSelectedEntry] = useState<MemoryEntry | null>(null);
  useEffect(() => { setExtra([]); setPage(0); setSelectedEntry(null); }, [data]);
  const entries = [...(data?.entries ?? []), ...extra];
  const selectedLoaded = entries.some(entry => entry.id === selected);
  useEffect(() => {
    if (!selected || !data || selectedLoaded) { setSelectedEntry(null); return; }
    let alive = true;
    memoryGet(selected).then(detail => { if (alive) setSelectedEntry(detail.entry); }).catch(() => {});
    return () => { alive = false; };
  }, [selected, selectedLoaded, data]);
  const more = async () => {
    setBusy(true); setFailure("");
    try {
      const next = await memoryList({ query: "", tag: "", sort: "title", page: page + 1, projectId, sessionId });
      setExtra(old => [...old, ...next.entries.filter(entry => !entries.some(e => e.id === entry.id))]);
      setPage(page + 1);
    } catch (e) { setFailure(memoryError(e)); }
    finally { setBusy(false); }
  };
  // Entries know their own stored grouping; the tree id only supplies a fallback. `__manual__` is the
  // tree's name for the empty stored session id, so entries dropped there go back to the manual group.
  const groupSession = sessionId === "__manual__" ? "" : sessionId;
  const row = (entry: { id: string; title: string; version: number; sessionId?: string }) => <KnowledgeTreeRow key={entry.id} name={entry.title} title={entry.title} route={`entry/${entry.id}`} depth={2} prose
    target={{ key: treeKey.memoryEntry(entry.id), kind: "memoryEntry", id: entry.id, title: entry.title, version: entry.version, sessionId: entry.sessionId ?? groupSession }}
    values={{ ...knowledgeSelection, memoryProject: projectId, memorySession: sessionId }} selected={entry.id === selected} icon={<Icons.diamond size={14} />} />;
  return <ul className="nb-tree-branches">
    {(!data || error) && <li><LoadState error={error} reload={reload} /></li>}
    {entries.map(row)}
    {selectedEntry && !selectedLoaded && selectedEntry.id === selected && row(selectedEntry)}
    {failure && <li><LoadState error={failure} reload={() => void more()} /></li>}
    {data && (page + 1) * data.pageSize < data.total && <li className="nb-tree-more"><button type="button" disabled={busy} onClick={() => void more()}>{t(busy ? "common.loading" : "nb.loadMore")}</button></li>}
  </ul>;
}
