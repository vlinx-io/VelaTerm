//! Project-scoped history import. The query URL supports reload, sharing and browser navigation.

import { useEffect, useState } from "react";
import { Backdrop } from "../components/Backdrop";
import { dateLocale, useT } from "../i18n";
import { discoverAgentSessions, importAgentSessions, type AgentHistory } from "../ipc/tree";
import { useTermStore } from "../store/termStore";
import { kindIconEl } from "./sessionViewers/sessionMeta";
import { kindLabel } from "./sessionMenuShared";

export function importSessionsUrl(projectId: string): string {
  const url = new URL(window.location.href);
  url.searchParams.set("importSessions", projectId);
  return `${url.pathname}${url.search}${url.hash}`;
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
    const url = new URL(window.location.href);
    url.searchParams.delete("importSessions");
    url.searchParams.delete("importSearch");
    navigate(`${url.pathname}${url.search}${url.hash}`);
  }} />;
}

function ImportSessions({ projectId, onClose }: { projectId: string; onClose: () => void }) {
  const t = useT();
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
  const visible = history?.sessions.filter((s) => `${s.title} ${s.kind} ${s.agentSessionId}`.toLocaleLowerCase().includes(query.toLocaleLowerCase())) || [];
  const available = visible.filter((s) => !s.imported);
  const allSelected = available.length > 0 && available.every((s) => selected.has(key(s)));
  const close = () => { if (!busy) onClose(); };
  const submit = async () => {
    if (!history || !selected.size || busy) return;
    setBusy(true); setError(""); setNotice("");
    try {
      const imported = await importAgentSessions(projectId, history.sessions.filter((s) => selected.has(key(s)))
        .map(({ kind, agentSessionId }) => ({ kind, agentSessionId })));
      await useTermStore.getState().loadTree();
      setNotice(t("importSessions.success", { count: imported.length }));
      refresh();
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  };
  return <Backdrop onClose={close}>
    <section role="dialog" aria-modal="true" aria-labelledby="import-sessions-title"
      style={{ width: 720, maxWidth: "94vw", maxHeight: "85vh", display: "flex", flexDirection: "column", gap: 12,
        background: "var(--bg-panel)", color: "var(--text-primary)", border: "1px solid var(--border)", borderRadius: 10, padding: 20 }}
      onKeyDown={(e) => { if (e.key === "Escape") { e.stopPropagation(); close(); } }}>
      <h2 id="import-sessions-title" style={{ margin: 0, fontSize: 16 }}>{t("importSessions.title")}</h2>
      <p style={{ margin: 0, fontSize: 12, color: "var(--text-secondary)" }}>{t("importSessions.description")}</p>
      {history && <div style={{ fontSize: 12, overflowWrap: "anywhere", color: "var(--text-muted)" }}>{history.directory}</div>}
      <div style={{ display: "flex", gap: 8 }}>
        <input className="vlx-input" autoFocus aria-label={t("importSessions.search")} placeholder={t("importSessions.search")}
          value={query} onChange={(e) => {
            setQuery(e.target.value);
            const url = new URL(window.location.href);
            if (e.target.value) url.searchParams.set("importSearch", e.target.value); else url.searchParams.delete("importSearch");
            window.history.replaceState(null, "", `${url.pathname}${url.search}${url.hash}`);
          }} style={{ flex: 1, minWidth: 0 }} />
        <button className="vlx-btn" onClick={refresh} disabled={busy || loading}>{t("common.refresh")}</button>
      </div>
      {error && <div role="alert" style={{ color: "var(--status-error)", fontSize: 12 }}>{error}</div>}
      {notice && <div role="status" style={{ fontSize: 12 }}>{notice}</div>}
      {history?.warnings.map((warning) => <div role="alert" key={warning} style={{ fontSize: 12, color: "var(--status-warning)" }}>{warning}</div>)}
      {loading ? <p role="status">{t("common.loading")}</p> : history && <>
        <label style={{ display: "flex", gap: 8, fontSize: 12 }}>
          <input type="checkbox" checked={allSelected} disabled={busy || !available.length} onChange={() => {
            setSelected((prev) => { const next = new Set(prev); available.forEach((s) => allSelected ? next.delete(key(s)) : next.add(key(s))); return next; });
          }} />{t("common.selectAll")}
        </label>
        <div style={{ overflow: "auto", minHeight: 80, maxHeight: "46vh", borderTop: "1px solid var(--border)" }}>
          {!visible.length && <p style={{ color: "var(--text-muted)", fontSize: 12 }}>{t("importSessions.empty")}</p>}
          {visible.map((s) => <label key={key(s)} style={{ display: "flex", alignItems: "center", gap: 10, padding: "12px 4px", borderBottom: "1px solid var(--border)", opacity: s.imported ? 0.6 : 1 }}>
            <input type="checkbox" aria-label={`${kindLabel(s.kind)}: ${s.title}`} disabled={busy || s.imported} checked={selected.has(key(s))} onChange={() => {
              setSelected((prev) => { const next = new Set(prev); if (next.has(key(s))) next.delete(key(s)); else next.add(key(s)); return next; });
            }} />
            {kindIconEl(s.kind, 18)}
            <span style={{ flex: 1, minWidth: 0 }}>
              <span title={s.title} style={{ display: "-webkit-box", WebkitLineClamp: 2, WebkitBoxOrient: "vertical", overflow: "hidden", fontSize: 13, overflowWrap: "anywhere" }}>{s.title}</span>
              <span style={{ display: "block", marginTop: 4, fontSize: 11, color: "var(--text-muted)", overflowWrap: "anywhere" }}>
                {kindLabel(s.kind)} · {s.agentSessionId}
              </span>
            </span>
            <span style={{ flexShrink: 0, fontSize: 11, color: "var(--text-muted)", textAlign: "right" }}>
              {s.imported ? t("importSessions.imported") : s.updatedAt > 0 ? new Date(s.updatedAt).toLocaleString(dateLocale()) : ""}
            </span>
          </label>)}
        </div>
      </>}
      <footer style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
        <button className="vlx-btn" onClick={close} disabled={busy}>{t("common.close")}</button>
        <button className="vlx-btn vlx-btn-primary" onClick={() => void submit()} disabled={busy || loading || !selected.size}>
          {busy ? t("common.loading") : t("importSessions.confirm", { count: selected.size })}
        </button>
      </footer>
    </section>
  </Backdrop>;
}
