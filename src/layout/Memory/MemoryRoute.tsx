//! Knowledge base surface: entry reading, provenance, editing and processing history.
import { useEffect, useRef } from "react";
import { useT } from "../../i18n";
import { MemoryLibrary } from "./MemoryLibrary";
import { MemoryCollections } from "./MemoryCollections";
import { MemoryDocument, MemoryEditor, MemorySourceView } from "./MemoryDocument";
import { MemoryCompile, MemoryJobs } from "./MemoryTasks";
import { useMemoryLocation } from "./navigation";
import "./memory.css";
import "./kbx.css";
import { KnowledgeVaultDialogs } from "../Notebook/VaultActions";
import { NotebookSurface } from "../Notebook/NotebookSurface";
import { KnowledgeBackLink } from "../Notebook/KnowledgeBack";

export function MemoryRoute() {
  const location = useMemoryLocation();
  const route = new URLSearchParams(location).get("memory");
  const legacy = ["notebooks/open", "notebooks/new"].includes(route ?? "");
  return <><KnowledgeVaultDialogs />{route && !legacy && <MemorySurface route={route} />}</>;
}
function MemorySurface({ route }: { route: string }) {
  const t = useT(); const ref = useRef<HTMLElement>(null);
  const [page, id = "", version] = route.split("/");
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    ref.current?.focus();
    return () => previous?.focus();
  }, []);
  const library = ["library", "entry", "history"].includes(page);
  const collections = page === "collections" || page === "collection";
  const notebook = page === "notebooks" || page === "notebook";
  const jobs = page === "jobs" || page === "job";
  return <section ref={ref} data-page={page} tabIndex={-1} className="memory-shell memory-tab-surface" role="tabpanel" aria-label={t(collections ? "memory.collections" : notebook ? "memory.title" : "memory.globalMemory")}>
      <div className="memory-body">
        {notebook ? <NotebookSurface route={route} /> : library ? <MemoryLibrary selected={id}>
          {id && <MemoryDocument key={`${page}/${id}/${version ?? ""}`} id={id} version={page === "history" ? Number(version) : undefined} />}
        </MemoryLibrary> : collections ? <MemoryCollections key={page} sessionId={page === "collection" ? id : ""} /> : jobs ? <MemoryJobs id={page === "job" ? id : undefined} /> : <div className="kbx-page">
          <div className="kbx-bar"><KnowledgeBackLink className="kbx-ibtn" /><div className="kbx-crumbs"><b>{t("memory.globalMemory")}</b></div></div>
          <main className="memory-main kbx-scroll" key={route}>
            {page === "compile" ? <MemoryCompile sessionId={id} /> : page === "source" ? <MemorySourceView id={id} /> : page === "edit" || page === "new" ? <MemoryEditor id={page === "edit" ? id : undefined} /> : <div className="memory-empty">{t("memory.notFound")}</div>}
          </main>
        </div>}
      </div>
    </section>;
}
