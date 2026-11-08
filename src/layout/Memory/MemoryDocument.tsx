import { useEffect, useRef, useState } from "react";
import { ContextMenu } from "../../components/ContextMenu";
import Icons from "../../components/Icons";
import { useT } from "../../i18n";
import { useTermStore } from "../../store/termStore";
import { memoryDelete, memoryGet, memoryOptions, memoryRestore, memorySave, memorySource, type MemoryDetail, type MemoryEntry, type MemoryEdit } from "../../ipc/memory";
import { kindLabel } from "../sessionMenuShared";
import type { SessionKind } from "../../types";
import { platform } from "../../platform";
import { writeTextFile } from "../../ipc/info";
import { isTauri } from "../../ipc/transport";
import { MemoryLink, memoryNavigate, memoryUrl } from "./navigation";
import { fullDate, runTime, LoadState, MemoryMarkdown, memoryError, memoryTime, useMemoryLoad } from "./shared";
import { WysiwygEditor, type WysiwygHandle } from "../CenterPane/doc/WysiwygEditor";
import "../CenterPane/doc/docTheme.css";
import { KnowledgeReferences } from "../Knowledge/KnowledgeReferences";

export function MemoryDocument({ id, version }: { id: string; version?: number }) {
  const t = useT(); const { data, error, reload } = useMemoryLoad(() => memoryGet(id, version), [id, version]);
  const [action, setAction] = useState<"delete" | "restore" | null>(null);
  const [busy, setBusy] = useState(false); const [failure, setFailure] = useState("");
  const editor = useRef<WysiwygHandle>(null);
  const [saved, setSaved] = useState<MemoryEntry | null>(null);
  const [dirty, setDirty] = useState(false);
  const saving = useRef(false);
  const [savedVersions, setSavedVersions] = useState<MemoryDetail["versions"]>([]);
  const [pending, setPending] = useState<{ url: string; replace: boolean } | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [tagsOpen, setTagsOpen] = useState(false);
  const rightCollapsed = useTermStore((state) => state.rightCollapsed);
  useEffect(() => { setTagsOpen(false); }, [id]);
  useEffect(() => {
    const unload = (event: BeforeUnloadEvent) => { if (dirty || busy) { event.preventDefault(); event.returnValue = ""; } };
    const navigate = (event: Event) => {
      if (!dirty && !busy) return;
      event.preventDefault();
      setPending((event as CustomEvent<{ url: string; replace: boolean }>).detail);
    };
    window.addEventListener("beforeunload", unload);
    window.addEventListener("memory:beforeNavigate", navigate);
    return () => { window.removeEventListener("beforeunload", unload); window.removeEventListener("memory:beforeNavigate", navigate); };
  }, [dirty, busy]);
  if (!data) return <LoadState error={error} reload={reload} />;
  const entry = version == null ? saved ?? data.entry : data.revision;
  if (!entry) return <p className="memory-empty">{t("memory.notFound")}</p>;
  // Revisions only matter once there is more than one, so a single version shows no number.
  const revised = version != null || data.versions.length + savedVersions.length > 1;
  // Tags on this entry alone connect it to nothing, so they wait behind the count.
  const shared = data.sharedTags;
  const tags = tagsOpen || !shared ? entry.tags : entry.tags.filter((tag) => shared.includes(tag));
  const save = async () => {
    const content = editor.current?.getMarkdown();
    if (content == null || busy || saving.current) return;
    saving.current = true;
    setBusy(true); setFailure("");
    try {
      const result = await memorySave({ id, version: entry.version, title: entry.title, summary: entry.summary, content, tags: entry.tags, related: entry.related });
      setSaved(result);
      setSavedVersions((versions) => [{ version: result.version, createdAt: result.updatedAt, author: "manual" }, ...versions]);
      setDirty(editor.current?.getMarkdown() !== content);
      window.dispatchEvent(new Event("memory:saved"));
    } catch (e) { setFailure(memoryError(e)); }
    finally { setBusy(false); saving.current = false; }
  };
  const perform = async () => {
    setBusy(true); setFailure("");
    try {
      if (action === "delete") { await memoryDelete(id, entry.version); memoryNavigate(memoryUrl("library"), false, true); }
      else if (version != null) { await memoryRestore(id, data.entry.version, version); memoryNavigate(memoryUrl(`entry/${id}`), false, true); }
      setAction(null);
    } catch (e) { setFailure(memoryError(e)); }
    finally { setBusy(false); }
  };
  const exportMarkdown = async () => {
    setFailure("");
    try {
    const permalink = (route: string) => { const url = new URL(location.href); url.search = ""; url.hash = ""; url.searchParams.set("memory", route); return url.href; };
    const body = version == null ? editor.current?.getMarkdown() ?? entry.content : entry.content;
    const content = `# ${entry.title}\n\n${entry.summary}\n\n${body}\n\n## ${t("memory.related")}\n\n${data.catalog.filter((e) => entry.related.includes(e.id)).map((e) => `- [${e.title}](${permalink(`entry/${e.id}`)})`).join("\n")}\n\n## ${t("memory.sources")}\n\n${data.sources.filter((s) => entry.sources.includes(s.id)).map((s) => `- ${s.sessionName} · ${s.kind} · ${memoryTime(s.createdAt)}\n  SHA-256: ${s.digest}\n  ${permalink(`source/${s.id}`)}`).join("\n")}\n`;
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
  return <article className="kbx-read-col" onKeyDown={(event) => {
    if (version == null && (event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "s") {
      event.preventDefault(); if (dirty) void save();
    }
  }}>
    <div className="kbx-read-a">
      <span className="g">{revised ? `v${entry.version} · ${fullDate(entry.updatedAt)}` : t("kb.updatedOn", fullDate(entry.updatedAt))}</span>
      {version == null ? <>
        {dirty && <button className="kbx-btn pri" disabled={busy} onClick={() => void save()}>{t(busy ? "common.loading" : "common.save")}</button>}
        <MemoryLink className="kbx-btn" route={`edit/${id}`}><Icons.rename size={14} aria-hidden="true" />{t("kb.editDetails")}</MemoryLink>
        <MemoryLink className="kbx-btn" route="notebooks/copy" values={{ memoryEntry: id }}><Icons.copy size={14} aria-hidden="true" />{t("nb.copyTo")}</MemoryLink>
        <button type="button" className="kbx-ibtn" aria-label={t("kb.moreActions")} title={t("kb.moreActions")} onClick={(event) => { const rect = event.currentTarget.getBoundingClientRect(); setMenu({ x: rect.right - 180, y: rect.bottom + 4 }); }}><Icons.more size={16} aria-hidden="true" /></button>
      </> : <>
        <MemoryLink className="kbx-btn" route={`entry/${id}`}>{t("common.cancel")}</MemoryLink>
        <button className="kbx-btn" disabled={version === data.entry.version} onClick={() => setAction("restore")}>{t("memory.restore")}</button>
      </>}
    </div>
    {menu && <ContextMenu x={menu.x} y={menu.y} onClose={() => setMenu(null)} items={[
      { label: t("memory.export"), onClick: () => void exportMarkdown() },
      { label: t("common.delete"), danger: true, disabled: busy, onClick: () => setAction("delete") },
    ]} />}
    {action && <div className="memory-confirm" role="alertdialog" aria-label={t(action === "delete" ? "memory.deleteConfirm" : "memory.restoreConfirm")}>
      <p>{t(action === "delete" ? "memory.deleteConfirm" : "memory.restoreConfirm")}</p><div className="memory-row">
        <button className="btn" disabled={busy} onClick={() => setAction(null)}>{t("common.cancel")}</button><button className="btn btn-primary" disabled={busy} onClick={() => void perform()}>{t(busy ? "common.loading" : "common.confirm")}</button>
      </div>
    </div>}
    {pending && <div className="memory-confirm" role="alertdialog" aria-label={t("memory.unsaved")}><p>{t("memory.unsaved")}</p><div className="memory-row"><button className="btn" onClick={() => setPending(null)}>{t("common.cancel")}</button><button className="btn" disabled={busy} onClick={() => memoryNavigate(pending.url, pending.replace, true)}>{t("common.confirm")}</button></div></div>}
    {failure && <p className="memory-error" role="alert">{failure}</p>}
    <h1>{entry.title}</h1><p className="kbx-lead">{entry.summary}</p>
    <div className="kbx-tags">{tags.map((tag) => <MemoryLink key={tag} className="kbx-chip" route="library" values={{ memoryTag: tag, memoryPage: null }}>{tag}</MemoryLink>)}{tags.length < entry.tags.length && <button type="button" className="kbx-more" title={t("kb.showAllTags")} aria-label={t("kb.showAllTags")} onClick={() => setTagsOpen(true)}>+{entry.tags.length - tags.length}</button>}</div>
    <div className={"kbx-prose" + (data.entry.content.trimStart().startsWith(`# ${entry.title}`) ? " kbx-dup-title" : "")}>
      {version == null ? <div className="docview memory-inline-editor" aria-label={t("memory.content")}>
        <WysiwygEditor ref={editor} defaultValue={data.entry.content} docPath="" textOnly onEdited={() => setDirty(true)} />
      </div> : <MemoryMarkdown content={entry.content} />}
    </div>
    {version == null && <KnowledgeReferences entryId={id} />}
    {/* These live in the right panel's Info tab; with the panel hidden they follow the body. */}
    {rightCollapsed && <div className="kbx-facts-inline"><EntryFacts id={id} version={version} data={data} entry={entry} savedVersions={savedVersions} /></div>}
  </article>;
}

/** Sources, related entries, backlinks and revisions of one entry, as right-panel sections. */
export function EntryFacts({ id, version, data, entry, savedVersions = [] }: { id: string; version?: number; data: MemoryDetail; entry: MemoryEntry; savedVersions?: MemoryDetail["versions"] }) {
  const t = useT();
  const sources = data.sources.filter((source) => entry.sources.includes(source.id));
  // Links usually run both ways, so entries this one points to and entries pointing here form one list.
  const related = [...data.catalog.filter((item) => entry.related.includes(item.id)), ...data.backlinks]
    .filter((item, index, all) => all.findIndex((other) => other.id === item.id) === index);
  const versions = [...savedVersions, ...data.versions.filter((item) => !savedVersions.some((saved) => saved.version === item.version))];
  return <>
    {sources.length > 0 && <section className="kbx-rsec"><h4>{sources.length === 1 ? t("kb.fromSession") : <>{t("memory.sources")}<i>{sources.length}</i></>}</h4>
      {sources.map((source) => <MemoryLink className="kbx-link" key={source.id} route={`source/${source.id}`}><Icons.archive size={14} aria-hidden="true" /><span>{source.sessionName}<small>{kindLabel(source.kind as SessionKind)} · {runTime(source.createdAt)}</small></span></MemoryLink>)}
    </section>}
    <FactLinks label={t("memory.related")} entries={related} />
    {(version != null || versions.length > 1) && <section className="kbx-rsec"><h4>{t("memory.history")}<i>{versions.length}</i></h4>
      {versions.map((item) => <MemoryLink key={item.version} route={`history/${id}/${item.version}`} className="kbx-link" aria-current={item.version === version ? "page" : undefined}>
        {item.author.includes(":") ? <Icons.bot size={14} aria-hidden="true" /> : <Icons.rename size={14} aria-hidden="true" />}
        <span>v{item.version}<small>{item.author.includes(":") ? kindLabel(item.author.split(":")[0] as SessionKind) : t(item.author === "restore" ? "memory.restore" : "common.edit")} · {runTime(item.createdAt)}</small></span>
      </MemoryLink>)}
    </section>}
  </>;
}

function FactLinks({ label, entries }: { label: string; entries: { id: string; title: string }[] }) {
  if (!entries.length) return null;
  return <section className="kbx-rsec"><h4>{label}<i>{entries.length}</i></h4>{entries.map((entry) => <MemoryLink className="kbx-link" key={entry.id} route={`entry/${entry.id}`}><Icons.diamond size={14} aria-hidden="true" /><span>{entry.title}</span></MemoryLink>)}</section>;
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
