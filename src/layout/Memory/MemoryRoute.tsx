//! Global Memory surface: thematic wiki reading, provenance, editing and compilation history.
import { useEffect, useRef } from "react";
import { useT } from "../../i18n";
import { MemoryLibrary } from "./MemoryLibrary";
import { MemoryDocument, MemoryEditor, MemorySourceView } from "./MemoryDocument";
import { MemoryCompile, MemoryJobs } from "./MemoryTasks";
import { MemoryLink, useMemoryLocation } from "./navigation";
import "./memory.css";

export function MemoryIcon({ size = 16 }: { size?: number }) {
  return <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d="M12 5v15M3 4.5c3.5-1 6-.5 9 1.5 3-2 5.5-2.5 9-1.5v14c-3.5-1-6-.5-9 1.5-3-2-5.5-2.5-9-1.5z" />
    <path d="M6 8h2m8 0h2M6 12h2m8 0h2" />
  </svg>;
}
export function MemoryRoute() {
  const location = useMemoryLocation();
  const route = new URLSearchParams(location).get("memory");
  if (!route) return null;
  return <MemorySurface route={route} />;
}
function MemorySurface({ route }: { route: string }) {
  const t = useT(); const ref = useRef<HTMLElement>(null);
  const [page, id = "", version] = route.split("/");
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    ref.current?.focus();
    return () => previous?.focus();
  }, []);
  const compact = page === "compile";
  const library = ["library", "entry", "history"].includes(page);
  return <section ref={ref} data-page={page} tabIndex={-1} className="memory-shell memory-tab-surface" role="tabpanel" aria-labelledby="memory-heading">
      <header className="memory-header">
        <div className="memory-brand"><MemoryIcon size={23} /><div><h2 id="memory-heading">{t("memory.title")}<span className="memory-badge">{t("common.experimental")}</span></h2><p>{t("memory.intro")}</p></div></div>
      </header>
      {!compact && <nav className="memory-nav" aria-label={t("memory.title")}>
        <MemoryLink route="library" className={library ? "active" : ""}>{t("memory.entries")}</MemoryLink>
        <MemoryLink route="jobs" values={{ memoryJobPage: null }} className={["jobs", "job"].includes(page) ? "active" : ""}>{t("memory.jobs")}</MemoryLink>
        <span className="memory-spacer" />
        <MemoryLink route="new" className="btn btn-primary">＋ {t("memory.new")}</MemoryLink>
      </nav>}
      <div className="memory-body">
        {library ? <><MemoryLibrary selected={id} /><main className="memory-main" key={`${page}/${id}/${version ?? ""}`}>
          {id ? <MemoryDocument id={id} version={page === "history" ? Number(version) : undefined} /> : <div className="memory-empty"><MemoryIcon size={40} /><p>{t("memory.emptyDetail")}</p></div>}
        </main></> : <main className="memory-main" key={route}>
          {page === "compile" ? <MemoryCompile sessionId={id} /> : page === "jobs" || page === "job" ? <MemoryJobs id={page === "job" ? id : undefined} /> : page === "source" ? <MemorySourceView id={id} /> : page === "edit" || page === "new" ? <MemoryEditor id={page === "edit" ? id : undefined} /> : <div className="memory-empty">{t("memory.notFound")}</div>}
        </main>}
      </div>
    </section>;
}
