//! Project-scoped history import. The query URL supports reload, sharing and browser navigation.

import { useEffect, useRef, useState } from "react";
import Icons from "../components/Icons";
import { Backdrop } from "../components/Backdrop";
import { dateLocale, useT } from "../i18n";
import { discoverAgentSessions, importAgentSessions, type AgentHistory } from "../ipc/tree";
import { useTermStore } from "../store/termStore";
import { kindIconEl } from "./sessionViewers/sessionMeta";
import { kindLabel } from "./sessionMenuShared";
import "./import-sessions.css";

export function importSessionsUrl(projectId: string): string {
  const url = new URL(window.location.href);
  if (projectId) url.searchParams.set("importSessions", projectId);
  else {
    url.searchParams.delete("importSessions");
    url.searchParams.delete("importSearch");
  }
  // A full URL also clears the query on desktop origins without a pathname.
  return url.href;
}

function navigate(url: string) {
  window.history.pushState(null, "", url);
  window.dispatchEvent(new PopStateEvent("popstate"));
}

export function openImportSessions(projectId: string) {
  navigate(importSessionsUrl(projectId));
}

export function ImportSessionsRoute() {
  const [projectId, setProjectId] = useState(() => new URLSearchParams(window.location.search).get("importSessions"));
  useEffect(() => {
    const update = () => setProjectId(new URLSearchParams(window.location.search).get("importSessions"));
    window.addEventListener("popstate", update);
    return () => window.removeEventListener("popstate", update);
  }, []);
  if (!projectId) return null;
  return <ImportSessions key={projectId} projectId={projectId} onClose={() => {
    navigate(importSessionsUrl(""));
  }} />;
}

function ImportSessions({ projectId, onClose }: { projectId: string; onClose: () => void }) {
  const t = useT();
  const dialogRef = useRef<HTMLElement>(null);
  const [previousFocus] = useState(() => document.activeElement);
  useEffect(() => () => {
    if (previousFocus instanceof HTMLElement && previousFocus.isConnected) previousFocus.focus();
  }, [previousFocus]);
  const [history, setHistory] = useState<AgentHistory | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [revision, setRevision] = useState(0);
  const [notice, setNotice] = useState("");
  const [query, setQuery] = useState(() => new URLSearchParams(window.location.search).get("importSearch") || "");
  useEffect(() => {
    const update = () => setQuery(new URLSearchParams(window.location.search).get("importSearch") || "");
    window.addEventListener("popstate", update);
    return () => window.removeEventListener("popstate", update);
  }, []);
  useEffect(() => {
    let alive = true;
    discoverAgentSessions(projectId).then((result) => {
      if (alive) { setHistory(result); setLoading(false); }
    }).catch((e) => { if (alive) { setError(String(e)); setLoading(false); } });
    return () => { alive = false; };
  }, [projectId, revision]);
  const refresh = () => {
    setLoading(true); setHistory(null); setError(""); setSelected(new Set()); setRevision((v) => v + 1);
  };
  const key = (s: { kind: string; agentSessionId: string }) => `${s.kind}:${s.agentSessionId}`;
  const visible = history?.sessions.filter((s) => `${s.title} ${s.kind} ${s.agentSessionId}`.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())) || [];
  const available = visible.filter((s) => !s.imported);
  const allSelected = available.length > 0 && available.every((s) => selected.has(key(s)));
  const partiallySelected = !allSelected && available.some((s) => selected.has(key(s)));
  const setSearch = (value: string) => {
    setQuery(value);
    const url = new URL(window.location.href);
    if (value) url.searchParams.set("importSearch", value); else url.searchParams.delete("importSearch");
    window.history.replaceState(window.history.state, "", url.href);
  };
  const close = () => { if (!busy) onClose(); };
  const submit = async () => {
    if (!history || !selected.size || busy) return;
    setBusy(true); setError(""); setNotice("");
    try {
      const imported = await importAgentSessions(projectId, history.sessions.filter((s) => selected.has(key(s)))
        .map(({ kind, agentSessionId }) => ({ kind, agentSessionId })));
      setSelected(new Set());
      setNotice(t("importSessions.success", { count: imported.length }));
      setHistory(await discoverAgentSessions(projectId));
      await useTermStore.getState().loadTree();
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  };
  return <Backdrop onClose={close}>
    <section ref={dialogRef} className="import-sessions" role="dialog" aria-modal="true"
      aria-labelledby="import-sessions-title" aria-describedby="import-sessions-description" tabIndex={-1}
      onKeyDown={(e) => {
        if (e.key === "Escape") { e.stopPropagation(); close(); }
        if (e.key !== "Tab") return;
        const controls = Array.from(dialogRef.current?.querySelectorAll<HTMLElement>(
          ':is(button, input, [href], [tabindex="0"]):not(:disabled)',
        ) || []);
        const first = controls[0], last = controls[controls.length - 1];
        if (!first) { e.preventDefault(); dialogRef.current?.focus(); }
        else if (e.shiftKey && (document.activeElement === first || document.activeElement === dialogRef.current)) {
          e.preventDefault(); last.focus();
        } else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
      }}>
      <header className="import-sessions-header">
        <div>
          <h2 id="import-sessions-title">{t("importSessions.title")}</h2>
          <p id="import-sessions-description">{t("importSessions.description")}</p>
        </div>
        <button type="button" className="vlx-btn import-sessions-icon" aria-label={t("common.close")}
          title={t("common.close")} onClick={close} disabled={busy}><Icons.close size={18} /></button>
      </header>
      <div className="import-sessions-toolbar">
        {history && <div className="import-sessions-directory" title={history.directory}>
          <Icons.folder size={15} /><span>{history.directory}</span>
        </div>}
        <div className="import-sessions-search-row">
          <div className="import-sessions-search">
            <Icons.search size={16} />
            <input className="vlx-input" autoFocus aria-label={t("importSessions.search")}
              placeholder={t("importSessions.search")} value={query} disabled={busy}
              onChange={(e) => setSearch(e.target.value)} />
          </div>
          <button type="button" className="vlx-btn" onClick={() => { setNotice(""); refresh(); }} disabled={busy || loading}>
            <Icons.restart size={14} />{t("common.refresh")}
          </button>
        </div>
        <div className="import-sessions-selection">
          <label>
            <input type="checkbox" checked={allSelected}
              ref={(node) => { if (node) node.indeterminate = partiallySelected; }}
              disabled={busy || loading || !available.length} onChange={() => {
                setSelected((prev) => {
                  const next = new Set(prev);
                  available.forEach((s) => allSelected ? next.delete(key(s)) : next.add(key(s)));
                  return next;
                });
              }} />{t("common.selectAll")}
          </label>
          <span aria-live="polite">{history && t("importSessions.results", { count: visible.length })}</span>
        </div>
      </div>
      <div className="import-sessions-content" aria-busy={loading || busy}>
        {error && <div className="import-sessions-message import-sessions-error" role="alert">{error}</div>}
        {notice && <div className="import-sessions-message import-sessions-success" role="status">
          <Icons.check size={16} /><span>{notice}</span>
        </div>}
        {history?.warnings.map((warning) => <div className="import-sessions-message import-sessions-warning" role="alert" key={warning}>{warning}</div>)}
        {loading ? <div className="import-sessions-empty" role="status">{t("common.loading")}</div> : history &&
          (!visible.length ? <div className="import-sessions-empty">
            <Icons.search size={28} />
            <p>{t(query.trim() ? "importSessions.empty" : "importSessions.noHistory")}</p>
            {query && <button type="button" className="vlx-btn" onClick={() => setSearch("")}>{t("importSessions.clearSearch")}</button>}
          </div> : <div className="import-sessions-list">
            {visible.map((s) => <label key={key(s)}
              className={`import-sessions-row${s.imported ? " is-imported" : ""}${selected.has(key(s)) ? " is-selected" : ""}`}>
              <input type="checkbox" aria-label={`${kindLabel(s.kind)}: ${s.title}`}
                disabled={busy || s.imported} checked={selected.has(key(s))} onChange={() => {
                  setSelected((prev) => { const next = new Set(prev); if (next.has(key(s))) next.delete(key(s)); else next.add(key(s)); return next; });
                }} />
              <span className="import-sessions-agent-icon">{kindIconEl(s.kind, 20)}</span>
              <span className="import-sessions-session">
                <span className="import-sessions-session-title" title={s.title}>{s.title}</span>
                <span className="import-sessions-meta">
                  <span>{kindLabel(s.kind)}</span>
                  {s.updatedAt > 0 && <time dateTime={new Date(s.updatedAt).toISOString()}>{new Date(s.updatedAt).toLocaleString(dateLocale())}</time>}
                  <span className="import-sessions-id" title={s.agentSessionId}>{s.agentSessionId}</span>
                </span>
              </span>
              {s.imported && <span className="import-sessions-badge"><Icons.check size={12} />{t("importSessions.imported")}</span>}
            </label>)}
          </div>)}
      </div>
      <footer className="import-sessions-footer">
        <div className="import-sessions-selected">
          <span role="status">{t("importSessions.selected", { count: selected.size })}</span>
          {selected.size > 0 && <button type="button" className="vlx-btn" onClick={() => setSelected(new Set())} disabled={busy}>
            {t("importSessions.clearSelection")}
          </button>}
        </div>
        <div className="import-sessions-actions">
          <button type="button" className="vlx-btn vlx-btn-primary" onClick={() => void submit()} disabled={busy || loading || !selected.size}>
            {busy ? t("common.loading") : t("importSessions.confirm", { count: selected.size })}
          </button>
        </div>
      </footer>
    </section>
  </Backdrop>;
}
