import Icons from "../../components/Icons";
import { useT } from "../../i18n";
import { memoryAttention, memoryCollections, memoryList } from "../../ipc/memory";
import { notebookTree, type NotebookOverview } from "../../ipc/notebook";
import { MemoryLink, memoryNavigate, memoryUrl } from "../Memory/navigation";
import { memoryError, shortDate } from "../Memory/shared";
import { KnowledgeHomeSearch } from "./KnowledgeHomeSearch";
import { knowledgeSelection } from "./knowledgeSelection";
import { noteFolder, noteTitle, notebookRoute, openVaultFile, useNotebookLoad } from "./shared";
import { showVaultAction } from "./VaultActions";
import { useNotebookImports } from "./importManager";

/** Rows in the home page's recently updated list. */
const RECENT_LIMIT = 10;

interface RecentItem { key: string; title: string; source: string; updatedAt: number; open: () => void; note: boolean }

/** Counts for the source cards, plus the newest entries and notes across every source. */
function useHomeStats(overview: NotebookOverview) {
  const vaultKey = overview.vaults.map((vault) => vault.id).join();
  return useNotebookLoad(async () => {
    const [entries, archived, attention, trees] = await Promise.all([
      memoryList({ query: "", tag: "", sort: "updated", page: 0 }),
      memoryCollections(),
      memoryAttention(),
      Promise.all(overview.vaults.map((vault) => notebookTree(vault.id, "", "", "updated"))),
    ]);
    const entrySource = (id?: string) => {
      for (const project of entries.projects) {
        const session = project.sessions.find((item) => item.id === (id || "__manual__"));
        if (session) return [project.name, session.name].filter(Boolean).join(" / ");
      }
      return "";
    };
    const archivedRows = archived.projects.flatMap((project) => project.sessions);
    const recent: RecentItem[] = [
      ...entries.entries.map((entry) => ({ key: `e:${entry.id}`, title: entry.title, source: entrySource(entry.sessionId), updatedAt: entry.updatedAt, note: false, open: () => memoryNavigate(memoryUrl(`entry/${entry.id}`, knowledgeSelection)) })),
      ...trees.flatMap((tree) => tree.entries.map((note) => ({ key: `n:${tree.vault.id}:${note.path}`, title: noteTitle(note.name), source: [tree.vault.name, noteFolder(note.path)].filter(Boolean).join(" / "), updatedAt: note.updatedAt, note: true, open: () => openVaultFile(note.absolutePath) }))),
    ].sort((a, b) => b.updatedAt - a.updatedAt).slice(0, RECENT_LIMIT);
    return {
      entries: entries.total,
      entrySessions: entries.projects.reduce((sum, project) => sum + project.sessions.length, 0),
      projects: entries.projects,
      notes: trees.reduce((sum, tree) => sum + tree.entries.length, 0),
      vaultNotes: new Map(trees.map((tree) => [tree.vault.id, tree.entries.length])),
      archived: archivedRows.length,
      archivedWithKnowledge: archivedRows.filter((row) => row.count > 0).length,
      lastArchived: archivedRows.reduce((latest, row) => Math.max(latest, row.archivedAt ?? 0), 0),
      archivedProjects: archived.projects.length,
      attention,
      recent,
    };
  }, [vaultKey], memoryError);
}

export function KnowledgeHome({ overview }: { overview: NotebookOverview }) {
  const t = useT();
  const running = [...useNotebookImports().values()];
  const { data } = useHomeStats(overview);
  const vaults = overview.vaults;
  return <main className="kbx-page">
    <div className="kbx-bar top">
      <div className="kbx-crumbs"><h1 className="kbx-bar-title">{t("memory.title")}</h1></div>
      <div className="kbx-acts">
        <button type="button" className="kbx-btn" onClick={() => showVaultAction("open")}><Icons.folderOpen size={14} />{t("nb.openVault")}</button>
        <button type="button" className="kbx-btn pri" onClick={() => showVaultAction("new")}><Icons.folderPlus size={14} />{t("nb.createVault")}</button>
      </div>
    </div>
    <div className="kbx-scroll"><div className="kbx-home">
    {running.map(state => {
      const vault = vaults.find(item => item.id === state.vaultId);
      return <MemoryLink key={state.vaultId} className="nb-import-chip nb-import-chip-wide" route={notebookRoute(state.vaultId, "imports")} title={state.current}>
        <Icons.upload size={12} /><span>{vault?.name ?? state.destination} · {t("nb.importProgress", String(state.done), String(state.files))}</span>
        <progress max={1} value={state.bytes ? state.sent / state.bytes : 1} />
      </MemoryLink>;
    })}
    <KnowledgeHomeSearch>
      <div className="kbx-cards">
        <section className="kbx-card">
          <MemoryLink className="h" route="library" values={knowledgeSelection}><Icons.layers size={16} className="kbx-ic-kb" aria-hidden="true" /><h5>{t("memory.globalMemory")}</h5><Icons.arrowRight size={14} aria-hidden="true" /></MemoryLink>
          <div className="num">{data ? data.entries.toLocaleString() : "\u00a0"}{data && <small>{t("kb.entriesFrom", String(data.entries), String(data.entrySessions))}</small>}</div>
          {data?.entries === 0 && <p className="d">{t("nb.generatedHint")}</p>}
          <div className="sub">
            {data?.projects.slice(0, 1).map((project) => <MemoryLink key={project.id} className="r" route="library" values={{ ...knowledgeSelection, memoryProject: project.id }}><Icons.folder size={14} aria-hidden="true" /><span className="nm">{project.name || t("memory.unknownProject")}</span><span className="n">{project.count}</span></MemoryLink>)}
            <MemoryLink className="r" route="jobs" values={knowledgeSelection}><Icons.history size={14} aria-hidden="true" /><span className="nm">{t("memory.jobs")}</span>{!!data?.attention && <span className="kbx-fail">{t("kb.needsAttention", String(data.attention))}</span>}</MemoryLink>
          </div>
        </section>
        <section className="kbx-card">
          <MemoryLink className="h" route={vaults.length === 1 ? notebookRoute(vaults[0].id) : "notebooks"} values={knowledgeSelection}><Icons.notebook size={16} aria-hidden="true" /><h5>{t("nb.localVaults")}</h5><Icons.arrowRight size={14} aria-hidden="true" /></MemoryLink>
          <div className="num">{data ? data.notes.toLocaleString() : "\u00a0"}{data && <small>{t("kb.notesIn", String(data.notes), String(vaults.length))}</small>}</div>
          {data?.notes === 0 && <p className="d">{t("kb.localHint")}</p>}
          <div className="sub">
            {vaults.slice(0, 2).map((vault) => <MemoryLink key={vault.id} className="r" route={notebookRoute(vault.id)} values={knowledgeSelection} title={vault.root}><Icons.notebook size={14} aria-hidden="true" /><span className="nm">{vault.name}</span><span className="n">{data?.vaultNotes.get(vault.id)?.toLocaleString()}</span></MemoryLink>)}
          </div>
        </section>
        <section className="kbx-card">
          <MemoryLink className="h" route="collections" values={knowledgeSelection}><Icons.archive size={16} className="kbx-ic-arch" aria-hidden="true" /><h5>{t("memory.collections")}</h5><Icons.arrowRight size={14} aria-hidden="true" /></MemoryLink>
          <div className="num">{data ? data.archived.toLocaleString() : "\u00a0"}{data && <small>{t("kb.sessionsIn", String(data.archived), String(data.archivedProjects))}</small>}</div>
          {data?.archived === 0 && <p className="d">{t("kb.archiveHint")}</p>}
          <div className="sub">
            <MemoryLink className="r" route="collections" values={{ ...knowledgeSelection, memoryCollectionKnowledge: "added" }}><Icons.layers size={14} className="kbx-ic-kb" aria-hidden="true" /><span className="nm">{t("archive.withKnowledge")}</span><span className="n">{data?.archivedWithKnowledge}</span></MemoryLink>
            <MemoryLink className="r" route="collections" values={knowledgeSelection}><Icons.clock size={14} aria-hidden="true" /><span className="nm">{t("kb.lastArchived")}</span><span className="n">{data?.lastArchived ? shortDate(data.lastArchived * 1000) : ""}</span></MemoryLink>
          </div>
        </section>
      </div>
      {!!data?.recent.length && <div>
        <div className="kbx-h2row"><b>{t("kb.recent")}</b></div>
        <div className="kbx-recent">
          {data.recent.map(item => <button type="button" key={item.key} className="kbx-rr" onClick={item.open}>
            {item.note ? <Icons.docLines size={14} aria-hidden="true" /> : <Icons.diamond size={14} className="kbx-ic-kb" aria-hidden="true" />}
            <span className="tt">{item.title}</span>
            <span className="w">{item.source}</span>
            <span className="w r">{shortDate(item.updatedAt)}</span>
          </button>)}
        </div>
      </div>}
    </KnowledgeHomeSearch>
  </div></div></main>;
}
