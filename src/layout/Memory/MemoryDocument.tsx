import { useEffect, useRef, useState } from "react";
import { useT } from "../../i18n";
import { memoryDelete, memoryGet, memoryOptions, memoryRestore, memorySave, memorySource, type MemoryDetail, type MemoryEdit } from "../../ipc/memory";
import { kindLabel } from "../sessionMenuShared";
import type { SessionKind } from "../../types";
import { platform } from "../../platform";
import { writeTextFile } from "../../ipc/info";
import { isTauri } from "../../ipc/transport";
import { MemoryLink, memoryNavigate, memoryUrl } from "./navigation";
import { LoadState, MemoryMarkdown, memoryError, memoryTime, useMemoryLoad } from "./shared";
import { KnowledgeReferences } from "../Knowledge/KnowledgeReferences";

export function MemoryDocument({ id, version }: { id: string; version?: number }) {
  const t = useT(); const { data, error, reload } = useMemoryLoad(() => memoryGet(id, version), [id, version]);
  const [action, setAction] = useState<"delete" | "restore" | null>(null);
  const [busy, setBusy] = useState(false); const [failure, setFailure] = useState("");
  if (!data) return <LoadState error={error} reload={reload} />;
  const entry = version == null ? data.entry : data.revision;
  if (!entry) return <p className="memory-empty">{t("memory.notFound")}</p>;
  const perform = async () => {
    setBusy(true); setFailure("");
    try {
      if (action === "delete") { await memoryDelete(id, data.entry.version); memoryNavigate(memoryUrl("library")); }
      else if (version != null) { await memoryRestore(id, data.entry.version, version); memoryNavigate(memoryUrl(`entry/${id}`)); }
      setAction(null);
    } catch (e) { setFailure(memoryError(e)); }
    finally { setBusy(false); }
  };
  const exportMarkdown = async () => {
    setFailure("");
    try {
    const permalink = (route: string) => { const url = new URL(location.href); url.search = ""; url.hash = ""; url.searchParams.set("memory", route); return url.href; };
    const content = `# ${entry.title}\n\n${entry.summary}\n\n${entry.content}\n\n## ${t("memory.related")}\n\n${data.catalog.filter((e) => entry.related.includes(e.id)).map((e) => `- [${e.title}](${permalink(`entry/${e.id}`)})`).join("\n")}\n\n## ${t("memory.sources")}\n\n${data.sources.filter((s) => entry.sources.includes(s.id)).map((s) => `- ${s.sessionName} · ${s.kind} · ${memoryTime(s.createdAt)}\n  SHA-256: ${s.digest}\n  ${permalink(`source/${s.id}`)}`).join("\n")}\n`;
    const fileName = `${entry.title.replace(/[\\/:*?"<>|]/g, "_")}.md`;
    if (isTauri || platform.env.isElectron) {
      const dest = await platform.dialog.saveFile({ defaultPath: fileName, filters: [{ name: "Markdown", extensions: ["md"] }] });
      if (dest) await writeTextFile(dest, content, null);
      return;
    }
    const blob = new Blob([content], { type: "text/markdown;charset=utf-8" });
    const url = URL.createObjectURL(blob); const a = document.createElement("a");
    a.href = url; a.download = fileName; document.body.appendChild(a); a.click(); a.remove(); setTimeout(() => URL.revokeObjectURL(url), 1000);
    } catch (e) { setFailure(memoryError(e)); }
  };
  return <article className="memory-document">
    <div className="memory-row memory-actions">
      <span className="memory-muted">v{entry.version} · {memoryTime(entry.updatedAt)}</span><span className="memory-spacer" />
      <button className="btn" onClick={() => void exportMarkdown()}>{t("memory.export")}</button>
      {version == null ? <><MemoryLink className="btn" route={`edit/${id}`}>{t("common.edit")}</MemoryLink><button className="btn" onClick={() => setAction("delete")}>{t("common.delete")}</button></> : <>
        <MemoryLink className="btn" route={`entry/${id}`}>{t("common.cancel")}</MemoryLink><button className="btn" disabled={version === data.entry.version} onClick={() => setAction("restore")}>{t("memory.restore")}</button>
      </>}
    </div>
    {action && <div className="memory-confirm" role="alertdialog" aria-label={t(action === "delete" ? "memory.deleteConfirm" : "memory.restoreConfirm")}>
      <p>{t(action === "delete" ? "memory.deleteConfirm" : "memory.restoreConfirm")}</p><div className="memory-row">
        <button className="btn" disabled={busy} onClick={() => setAction(null)}>{t("common.cancel")}</button><button className="btn btn-primary" disabled={busy} onClick={() => void perform()}>{t(busy ? "common.loading" : "common.confirm")}</button>
      </div>
    </div>}
    {failure && <p className="memory-error" role="alert">{failure}</p>}
    <h1>{entry.title}</h1><p className="memory-lead">{entry.summary}</p>
    <div className="memory-tags">{entry.tags.map((tag) => <MemoryLink key={tag} route="library" values={{ memoryTag: tag, memoryPage: null }}>{tag}</MemoryLink>)}</div>
    <MemoryMarkdown content={entry.content} />
    {version == null && <KnowledgeReferences entryId={id} />}
    <div className="memory-references">
      <ReferenceList label={t("memory.related")} entries={data.catalog.filter((e) => entry.related.includes(e.id))} />
      <ReferenceList label={t("memory.backlinks")} entries={data.backlinks} />
      <section><h3>{t("memory.sources")} · {entry.sources.length}</h3>
        {data.sources.filter((s) => entry.sources.includes(s.id)).map((s) => <MemoryLink className="memory-reference" key={s.id} route={`source/${s.id}`}><strong>{s.sessionName}</strong><small>{kindLabel(s.kind as SessionKind)} · {memoryTime(s.createdAt)}</small></MemoryLink>)}
      </section>
      <section><h3>{t("memory.history")}</h3><div className="memory-history">
        {data.versions.map((v) => <MemoryLink key={v.version} route={`history/${id}/${v.version}`} className={v.version === version ? "active" : ""}>
          v{v.version} · {memoryTime(v.createdAt)} <small>{v.author.includes(":") ? kindLabel(v.author.split(":")[0] as SessionKind) : t(v.author === "restore" ? "memory.restore" : "common.edit")}</small>
        </MemoryLink>)}
      </div></section>
    </div>
  </article>;
}
function ReferenceList({ label, entries }: { label: string; entries: { id: string; title: string }[] }) {
  if (!entries.length) return null;
  return <section><h3>{label}</h3>{entries.map((e) => <MemoryLink className="memory-reference" key={e.id} route={`entry/${e.id}`}>{e.title} ↗</MemoryLink>)}</section>;
}

export function MemoryEditor({ id }: { id?: string }) {
  const { data, error, reload } = useMemoryLoad(async () => id ? memoryGet(id) : { catalog: (await memoryOptions()).catalog }, [id]);
  if (!data) return <LoadState error={error} reload={reload} />;
  return <EditorForm id={id} data={data} />;
}
function EditorForm({ id, data }: { id?: string; data: MemoryDetail | { catalog: { id: string; title: string }[] } }) {
  const t = useT(); const original = "entry" in data ? data.entry : null;
  const [form, setForm] = useState<MemoryEdit>({ id: id ?? null, version: original?.version ?? 0, title: original?.title ?? "", summary: original?.summary ?? "", content: original?.content ?? "", tags: original?.tags ?? [], related: original?.related ?? [] });
  const [tags, setTags] = useState(form.tags.join(", "));
  const [busy, setBusy] = useState(false); const [failure, setFailure] = useState("");
  const [preview, setPreview] = useState(false);
  const [pending, setPending] = useState<{ url: string; replace: boolean } | null>(null);
  const saving = useRef(false);
  const dirty = form.title !== (original?.title ?? "") || form.summary !== (original?.summary ?? "") || form.content !== (original?.content ?? "") || tags !== (original?.tags ?? []).join(", ") || JSON.stringify(form.related) !== JSON.stringify(original?.related ?? []);
  useEffect(() => {
    const guard = (e: BeforeUnloadEvent) => { if (dirty) { e.preventDefault(); e.returnValue = ""; } };
    window.addEventListener("beforeunload", guard); return () => window.removeEventListener("beforeunload", guard);
  }, [dirty]);
  useEffect(() => {
    const guard = (event: Event) => {
      if (!dirty || saving.current) return;
      event.preventDefault(); setPending((event as CustomEvent<{ url: string; replace: boolean }>).detail);
    };
    window.addEventListener("memory:beforeNavigate", guard);
    return () => window.removeEventListener("memory:beforeNavigate", guard);
  }, [dirty]);
  const save = async () => {
    saving.current = true;
    setBusy(true); setFailure("");
    try { const entry = await memorySave({ ...form, tags: tags.split(/[,，]/).map((tag) => tag.trim()).filter(Boolean) }); memoryNavigate(memoryUrl(`entry/${entry.id}`)); }
    catch (e) { setFailure(memoryError(e)); }
    finally { setBusy(false); saving.current = false; }
  };
  return <form className="memory-document memory-editor" onSubmit={(e) => { e.preventDefault(); void save(); }}>
    <div className="memory-row"><h2>{t(id ? "common.edit" : "memory.new")}</h2><span className="memory-spacer" /><MemoryLink className="btn" route={id ? `entry/${id}` : "library"}>{t("common.cancel")}</MemoryLink><button className="btn btn-primary" type="submit" disabled={busy}>{t(busy ? "common.loading" : "common.save")}</button></div>
    {failure && <p className="memory-error" role="alert">{failure}</p>}
    {pending && <div className="memory-confirm" role="alertdialog" aria-label={t("memory.unsaved")}><p>{t("memory.unsaved")}</p><div className="memory-row"><button type="button" className="btn" onClick={() => setPending(null)}>{t("common.cancel")}</button><button type="button" className="btn" onClick={() => memoryNavigate(pending.url, pending.replace, true)}>{t("common.confirm")}</button></div></div>}
    <label>{t("memory.titleField")}<input className="input" autoFocus required maxLength={200} value={form.title} onChange={(e) => setForm({ ...form, title: e.target.value })} /></label>
    <label>{t("memory.summary")}<textarea className="input" rows={2} maxLength={2000} value={form.summary} onChange={(e) => setForm({ ...form, summary: e.target.value })} /></label>
    <label>{t("memory.tags")}<input className="input" value={tags} onChange={(e) => setTags(e.target.value)} /></label>
    <div className="memory-row"><span>{t("memory.content")}</span><span className="memory-spacer" /><button type="button" className="btn" onClick={() => setPreview(!preview)}>{t(preview ? "common.edit" : "panel.preview")}</button></div>
    {preview ? <MemoryMarkdown content={form.content} /> : <textarea className="input memory-content-input" aria-label={t("memory.content")} required value={form.content} onChange={(e) => setForm({ ...form, content: e.target.value })} />}
    <fieldset><legend>{t("memory.related")}</legend><div className="memory-related-options">{data.catalog.filter((e) => e.id !== id).map((entry) => <label key={entry.id}>
      <input type="checkbox" checked={form.related.includes(entry.id)} onChange={(e) => setForm({ ...form, related: e.target.checked ? [...form.related, entry.id] : form.related.filter((value) => value !== entry.id) })} />{entry.title}
    </label>)}</div></fieldset>
  </form>;
}

export function MemorySourceView({ id }: { id: string }) {
  const t = useT(); const { data, error, reload } = useMemoryLoad(() => memorySource(id), [id]);
  if (!data) return <LoadState error={error} reload={reload} />;
  return <article className="memory-document"><span className="memory-muted">{t("memory.source")}</span><h1>{data.sessionName}</h1><p>{t("memory.sourceNote")}</p><p className="memory-muted">{kindLabel(data.kind as SessionKind)} · {memoryTime(data.createdAt)}</p><details><summary>SHA-256</summary><code>{data.digest}</code></details><pre className="memory-source-text">{data.content}</pre></article>;
}
