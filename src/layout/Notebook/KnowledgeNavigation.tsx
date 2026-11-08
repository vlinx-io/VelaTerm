import { useEffect, useState } from "react";
import Icons from "../../components/Icons";
import { useT } from "../../i18n";
import { notebookOverview, notebookTree } from "../../ipc/notebook";
import { MemoryLink, useMemoryLocation } from "../Memory/navigation";
import { memoryNavigate, memoryUrl } from "../Memory/navigation";
import { MemoryDirectory } from "../Memory/MemoryDirectory";
import { MemoryCollectionsDirectory } from "../Memory/MemoryCollections";
import { memoryAttention, memoryCollections, memoryList } from "../../ipc/memory";
import { useHomeSearch } from "./KnowledgeHomeSearch";
import { useTermStore } from "../../store/termStore";
import { KnowledgeTreeRow, knowledgeSelection, treeKey, useKnowledgeTree } from "./KnowledgeTreeRow";
import { KnowledgeTreeProvider } from "./KnowledgeTreeActions";
import { KnowledgeSearchResults, useSearchKeys } from "./KnowledgeSearch";
import { NotebookError, notebookRoute, openVaultFile, useNotebookLoad } from "./shared";
import { showVaultAction } from "./VaultActions";
import "./notebook.css";
import "../Memory/kbx.css";

/** The knowledge tree. `heading` is off in the left sidebar, whose own header carries the title and vault buttons. */
export function KnowledgeNavigation({ heading = true }: { heading?: boolean }) {
  return <KnowledgeTreeProvider><KnowledgeNavigationTree heading={heading} /></KnowledgeTreeProvider>;
}

function KnowledgeNavigationTree({ heading }: { heading: boolean }) {
  const t = useT();
  const search = useMemoryLocation();
  const params = new URLSearchParams(search);
  const [page, id] = (params.get("memory") ?? "").split('/');
  const { data, error, reload } = useNotebookLoad(notebookOverview, [page, id]);
  const vaultKey = data?.vaults.map((vault) => vault.id).join() ?? "";
  const stats = useNotebookLoad(() => Promise.all([
    memoryList({ query: "", tag: "", sort: "updated", page: 0 }),
    memoryCollections(),
    memoryAttention(),
    Promise.all((data?.vaults ?? []).map((vault) => notebookTree(vault.id).then((tree) => [vault.id, tree.entries.length] as const))),
  ]).then(([entries, archived, attention, notes]) => ({
    entries: entries.total,
    sessions: archived.projects.reduce((sum, project) => sum + project.sessions.length, 0),
    attention,
    notes: new Map(notes),
  })), [page, vaultKey]);
  const collections = page === "collections" || page === "collection";
  const selectedRoot = collections ? "collections" : page === "notebook" ? id : "";
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set(selectedRoot ? [selectedRoot] : []));
  const open = (key: string) => setExpanded(old => old.has(key) ? old : new Set(old).add(key));
  const toggle = (key: string) => setExpanded(old => { const next = new Set(old); if (next.has(key)) next.delete(key); else next.add(key); return next; });
  useEffect(() => { if (selectedRoot) open(selectedRoot); }, [selectedRoot, search]);
  useEffect(() => {
    window.addEventListener("notebook:vaultsChanged", reload);
    window.addEventListener("focus", reload);
    return () => { window.removeEventListener("notebook:vaultsChanged", reload); window.removeEventListener("focus", reload); };
  }, [reload]);
  // A query replaces the tree with hits from session knowledge and local notes.
  const [query, setQuery] = useState("");
  const lookup = useHomeSearch(query);
  const entries = lookup.entries?.entries ?? [];
  const hits = lookup.notes?.entries ?? [];
  const openHit = (index: number) => {
    if (index < entries.length) memoryNavigate(memoryUrl(`entry/${entries[index].id}`));
    else {
      const hit = hits[index - entries.length];
      if (hit) openVaultFile(hit.absolutePath);
    }
  };
  const keys = useSearchKeys(entries.length + hits.length, openHit, () => setQuery(""));
  const searching = query.trim().length > 0;
  return <div className={"nb-navigation" + (heading ? "" : " no-heading")}>
    {heading && <header className="nb-navigation-heading"><MemoryLink route="notebooks" values={knowledgeSelection}>{t("nb.vaults")}</MemoryLink><div>
      <button className="nb-icon-button" title={t("nb.openVault")} aria-label={t("nb.openVault")} onClick={() => showVaultAction("open")}><Icons.folderOpen /></button>
      <button className="nb-icon-button" title={t("nb.createVault")} aria-label={t("nb.createVault")} onClick={() => showVaultAction("new")}><Icons.folderPlus /></button>
    </div></header>}
    <div className="nb-navigation-search">
      <Icons.search size={12} aria-hidden="true" />
      <input value={query} aria-label={t("kb.searchAll")} placeholder={t("kb.searchAll")} onKeyDown={keys.onKeyDown} onChange={(e) => setQuery(e.target.value)} />
    </div>
    {searching ? <div className="nb-knowledge-tree">
      {entries.length > 0 && <ul className="nb-tree-branches" aria-label={t("memory.globalMemory")}>
        {entries.map((entry, index) => <KnowledgeTreeRow key={entry.id} name={entry.title} route={`entry/${entry.id}`} values={knowledgeSelection} depth={0} prose selected={keys.active === index} icon={<Icons.diamond size={14} className="kbx-ic-kb" />} />)}
      </ul>}
      <KnowledgeSearchResults query={query} result={lookup.notes} error={lookup.error} active={keys.active - entries.length} onActive={(index) => keys.setActive(index + entries.length)} onOpen={(index) => openHit(index + entries.length)} onOpenRelated={(related)=>openVaultFile(related.absolutePath)} compact />
    </div> : <nav className="nb-knowledge-tree" aria-label={t("nb.vaults")}>
      <ul className="nb-tree-branches">
        <KnowledgeTreeRow name={t("kb.overview")} route="notebooks" values={knowledgeSelection} depth={0} icon={<Icons.home size={14} />} selected={!page || page === "notebooks"} />
      </ul>
      <div className="kbx-lab"><MemoryLink route="library" values={knowledgeSelection}>{t("memory.globalMemory")}</MemoryLink><i>{stats.data?.entries ?? ""}</i></div>
      <MemoryDirectory />
      <ul className="nb-tree-branches">
        <KnowledgeTreeRow name={t("memory.jobs")} route="jobs" values={{ ...knowledgeSelection, memoryJobPage: null }} depth={0} icon={<Icons.history size={14} />} attention={stats.data?.attention} selected={page === "jobs" || page === "job"} />
      </ul>
      <div className="kbx-lab"><span>{t("nb.localVaults")}</span><i>{data?.vaults.length ?? ""}</i></div>
      <ul className="nb-tree-branches">
        {data?.vaults.map(v => <KnowledgeTreeRow key={v.id} name={v.name} route={notebookRoute(v.id)} values={knowledgeSelection} depth={0} title={v.root} icon={<Icons.notebook size={14} />} count={stats.data?.notes.get(v.id)}
          target={{ key: treeKey.vault(v.id), kind: "vault", id: v.id, name: v.name, root: v.root }}
          selected={page === "notebook" && id === v.id && !params.has("memoryPath") && !params.has("memoryFolder") && !params.has("memoryFilter")}
          expanded={expanded.has(v.id)} onToggle={() => toggle(v.id)} onOpen={() => open(v.id)}>
          <NotebookDirectory id={v.id} />
        </KnowledgeTreeRow>)}
        {data && !data.vaults.length && <li className="nb-tree-empty">{t("nb.empty")}</li>}
      </ul>
      <div className="kbx-lab"><span>{t("kb.sources")}</span></div>
      <ul className="nb-tree-branches">
        <KnowledgeTreeRow name={t("memory.collections")} route="collections" values={{ ...knowledgeSelection, memoryCollectionProject: null, memoryCollectionQuery: null }} depth={0} icon={<Icons.archive size={14} />} count={stats.data?.sessions}
          selected={collections && !params.has("memoryCollectionProject") && page === "collections"}
          expanded={expanded.has("collections")} onToggle={() => toggle("collections")} onOpen={() => open("collections")}>
          <MemoryCollectionsDirectory baseDepth={1} />
        </KnowledgeTreeRow>
      </ul>
      {error && <NotebookError error={error} retry={reload} />}
      {!data && !error && <NotebookError error="" />}
    </nav>}
  </div>;
}

/** The Archived Sessions section of the sidebar: every session, then the same project tree the Knowledge section lists under Sources. */
export function ArchiveNavigation() {
  const t = useT();
  const search = useMemoryLocation();
  const params = new URLSearchParams(search);
  const page = params.get("memory") ?? "";
  const [query, setQuery] = useState(params.get("memoryCollectionQuery") ?? "");
  useEffect(() => {
    const current = new URLSearchParams(window.location.search).get("memoryCollectionQuery") ?? "";
    if (query === current) return;
    const timer = setTimeout(() => memoryNavigate(memoryUrl("collections", { ...knowledgeSelection, memoryCollectionProject: null, memoryCollectionQuery: query || null }), true), 250);
    return () => clearTimeout(timer);
  }, [query]);
  const stats = useNotebookLoad(memoryCollections, []);
  const projects = stats.data?.projects ?? [];
  const total = projects.reduce((sum, project) => sum + project.sessions.length, 0);
  return <div className="nb-navigation no-heading">
    <div className="nb-navigation-search">
      <Icons.search size={12} aria-hidden="true" />
      <input value={query} aria-label={t("archive.searchPlaceholder")} placeholder={t("archive.searchPlaceholder")} onChange={(e) => setQuery(e.target.value)} />
    </div>
    <nav className="nb-knowledge-tree" aria-label={t("memory.collections")}>
      <ul className="nb-tree-branches">
        <KnowledgeTreeRow name={t("archive.allSessions")} route="collections" values={{ ...knowledgeSelection, memoryCollectionProject: null, memoryCollectionQuery: null }} depth={0} icon={<Icons.archive size={14} />} count={stats.data ? total : undefined}
          selected={page === "collections" && !params.has("memoryCollectionProject")} />
      </ul>
      <div className="kbx-lab"><span>{t("archive.projects")}</span><i>{stats.data ? projects.length : ""}</i></div>
      <MemoryCollectionsDirectory />
    </nav>
  </div>;
}

/** Forward-slash form used to compare native absolute paths independent of the platform separator. */
const norm = (path: string) => path.replace(/\\/g, "/");
/** Path relative to a vault root, or empty when the absolute path belongs to another location. */
function relativeTo(root: string, absolute: string): string {
  const base = norm(root).replace(/\/+$/, "");
  const target = norm(absolute);
  return base && target.startsWith(`${base}/`) ? target.slice(base.length + 1) : "";
}

function NotebookDirectory({ id }: { id: string }) {
  const t = useT();
  const search = useMemoryLocation();
  const actions = useKnowledgeTree();
  const params = new URLSearchParams(search);
  const active = params.get("memory")?.split('/').slice(0, 2).join('/') === notebookRoute(id);
  const folder = active ? params.get("memoryFolder") ?? "" : "";
  const activeDocPath = useTermStore(state => state.activeTabId ? state.docTabs[state.activeTabId]?.path ?? "" : "");
  const { data, error, reload } = useNotebookLoad(() => notebookTree(id), [id, search, actions?.revision ?? 0]);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const open = (key: string) => setExpanded(old => old.has(key) ? old : new Set(old).add(key));
  const toggle = (key: string) => setExpanded(old => { const next = new Set(old); if (next.has(key)) next.delete(key); else next.add(key); return next; });
  useEffect(() => {
    const selected = relativeTo(data?.vault.root ?? "", activeDocPath) || folder;
    if (!selected) return;
    setExpanded(old => {
      const next = new Set(old), parts = selected.split('/');
      for (let i = 1; i < parts.length; i++) next.add(parts.slice(0, i).join('/'));
      if (folder) next.add(folder);
      return next;
    });
  }, [data?.vault.root, activeDocPath, folder]);
  useEffect(() => {
    const update = () => { if (!document.hidden) reload(); };
    const timer = setInterval(update, 5000);
    window.addEventListener("focus", update);
    return () => { clearInterval(timer); window.removeEventListener("focus", update); };
  }, [reload]);
  const renderTree = (parent = "", depth = 1): React.ReactNode => data?.nodes.filter(n => (n.path.includes('/') ? n.path.slice(0, n.path.lastIndexOf('/')) : "") === parent).map(n => {
    const isFolder = n.kind === "folder";
    return <KnowledgeTreeRow key={n.path} prose={!isFolder} name={n.kind === "note" ? n.name.replace(/\.md$/i, "") : n.name} route={isFolder ? notebookRoute(id) : undefined} title={n.path} depth={depth}
      target={{ key: treeKey.notebookNode(id, n.path), kind: "notebookNode", vaultId: id, path: n.path, name: n.name, nodeKind: n.kind }}
      values={isFolder ? { ...knowledgeSelection, memoryPath: null, memoryFolder: n.path } : undefined}
      onActivate={isFolder ? undefined : () => useTermStore.getState().openDocTab(n.absolutePath)}
      selected={isFolder ? n.path === folder : norm(n.absolutePath) === norm(activeDocPath)}
      icon={isFolder ? <Icons.folder size={14} /> : n.kind === "note" ? <Icons.docLines size={14} /> : <Icons.file size={14} />}
      expanded={isFolder && expanded.has(n.path)} onToggle={isFolder ? () => toggle(n.path) : undefined} onOpen={isFolder ? () => open(n.path) : undefined}>
      {n.kind === "folder" && <ul className="nb-tree-branches">{renderTree(n.path, depth + 1)}</ul>}
    </KnowledgeTreeRow>;
  });
  return <ul className="nb-tree-branches nb-file-tree">
    {(!data || error) && <li><NotebookError error={error} retry={reload} /></li>}
    {data && renderTree()}
    {data && !data.nodes.length && <li className="nb-tree-empty">{t("nb.chooseNote")}</li>}
  </ul>;
}
