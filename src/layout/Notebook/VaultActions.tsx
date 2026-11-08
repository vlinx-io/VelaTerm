import { useEffect, useRef, useState } from "react";
import { Backdrop } from "../../components/Backdrop";
import { useT } from "../../i18n";
import { notebookOverview, notebookRegister } from "../../ipc/notebook";
import { platform } from "../../platform";
import { cardStyle, joinPath, ServerBrowserView, useServerBrowser } from "../../remote/ServerFileBrowser";
import { memoryNavigate, memoryUrl, useMemoryLocation } from "../Memory/navigation";
import { NotebookError, notebookError, notebookRoute, useNotebookLoad } from "./shared";

export function showVaultAction(action: "open" | "new") {
  const url = new URL(location.href);
  url.searchParams.set("kbAction", action);
  memoryNavigate(url.href);
}

function dismissVaultAction() {
  const url = new URL(location.href);
  url.searchParams.delete("kbAction");
  memoryNavigate(url.href, true);
}

function openRegisteredVault(id: string) {
  window.dispatchEvent(new Event("notebook:vaultsChanged"));
  const url = new URL(memoryUrl(notebookRoute(id)));
  // Filters and selected files belong to the previously selected knowledge base.
  [...url.searchParams.keys()].filter(key => key.startsWith("memory") && key !== "memory").forEach(key => url.searchParams.delete(key));
  url.searchParams.delete("kbAction");
  dismissVaultAction();
  memoryNavigate(url.href);
}

/** Dialog URLs preserve the underlying editor; former landing-page URLs redirect here. */
export function KnowledgeVaultDialogs() {
  const search = useMemoryLocation();
  const params = new URLSearchParams(search);
  const route = params.get("memory");
  const action = params.get("kbAction");
  useEffect(() => {
    if (!["notebooks/open", "notebooks/new"].includes(route ?? "")) return;
    const url = new URL(memoryUrl("notebooks"));
    url.searchParams.set("kbAction", route!.split('/')[1]);
    memoryNavigate(url.href, true);
  }, [route]);
  if (action === "open") return <OpenVaultDialog />;
  if (action === "new") return <NewVaultDialog />;
  return null;
}

function VaultModal({ title, busy, children }: { title: string; busy: boolean; children: React.ReactNode }) {
  const t = useT();
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    if (!ref.current?.contains(document.activeElement)) ref.current?.focus();
    return () => previous?.focus();
  }, []);
  const close = () => { if (!busy) dismissVaultAction(); };
  return <Backdrop onClose={close}>
    <div ref={ref} tabIndex={-1} role="dialog" aria-modal="true" aria-label={title} className="nb-vault-modal" style={cardStyle} onClick={e => e.stopPropagation()} onKeyDown={e => {
      if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); close(); }
      if (e.key === "Tab") {
        const controls = Array.from(ref.current?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled), a[href], [tabindex="0"]') ?? []).filter(el => el.getClientRects().length);
        const first = controls[0], last = controls[controls.length - 1];
        if (e.shiftKey && (document.activeElement === first || document.activeElement === ref.current)) { e.preventDefault(); last?.focus(); }
        else if (!e.shiftKey && (document.activeElement === last || document.activeElement === ref.current)) { e.preventDefault(); first?.focus(); }
      }
    }}>
      <header><h2>{title}</h2><button type="button" className="nb-icon-button" aria-label={t("common.cancel")} title={t("common.cancel")} disabled={busy} onClick={close}>×</button></header>
      {children}
    </div>
  </Backdrop>;
}

function OpenVaultDialog() {
  const t = useT();
  const native = !platform.env.isBrowser;
  const browser = useServerBrowser(!native);
  const started = useRef(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const open = async (path: string) => {
    setBusy(true); setError("");
    try {
      const vault = await notebookRegister(path, false);
      if (new URLSearchParams(location.search).get("kbAction") === "open") openRegisteredVault(vault.id);
    } catch (e) { setError(notebookError(e)); }
    finally { setBusy(false); }
  };
  const choose = async () => {
    setBusy(true); setError("");
    try {
      const path = await platform.dialog.pickDirectory();
      if (new URLSearchParams(location.search).get("kbAction") !== "open") return;
      if (path) await open(path); else dismissVaultAction();
    } catch (e) { setError(notebookError(e)); }
    finally { setBusy(false); }
  };
  useEffect(() => {
    if (native && !started.current) { started.current = true; void choose(); }
  }, [native]);
  // The system picker is the only opening UI on desktop; show a retry dialog only on failure.
  if (native && !error) return null;
  return <VaultModal title={t("nb.openVault")} busy={busy}>
    {!native && <ServerBrowserView browser={browser} />}
    {error && <NotebookError error={error} />}
    <footer><button className="btn" disabled={busy} onClick={dismissVaultAction}>{t("common.cancel")}</button><button className="btn btn-primary" disabled={busy || !native && !browser.selectedDir} onClick={() => void (native ? choose() : open(browser.selectedDir))}>{t(busy ? "common.loading" : native ? "common.retry" : "common.open")}</button></footer>
  </VaultModal>;
}

function NewVaultDialog() {
  const t = useT();
  const { data, error, reload } = useNotebookLoad(notebookOverview, []);
  if (!data) return <VaultModal title={t("nb.createVault")} busy={false}><NotebookError error={error} retry={reload} /></VaultModal>;
  return <CreateVaultForm defaultRoot={data.defaultRoot ?? ""} />;
}

function CreateVaultForm({ defaultRoot }: { defaultRoot: string }) {
  const t = useT();
  const split = Math.max(defaultRoot.lastIndexOf('/'), defaultRoot.lastIndexOf('\\'));
  const [parent, setParent] = useState(defaultRoot.slice(0, split + 1));
  const [name, setName] = useState(defaultRoot.slice(split + 1));
  const [browsing, setBrowsing] = useState(false);
  const browser = useServerBrowser(browsing);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const chooseParent = async () => {
    if (platform.env.isBrowser) { setBrowsing(true); return; }
    try { const path = await platform.dialog.pickDirectory(); if (path) setParent(path); }
    catch (e) { setError(notebookError(e)); }
  };
  const submit = async () => {
    setBusy(true); setError("");
    try {
      const vault = await notebookRegister(joinPath(parent, name), true);
      if (new URLSearchParams(location.search).get("kbAction") === "new") openRegisteredVault(vault.id);
    } catch (e) { setError(notebookError(e)); }
    finally { setBusy(false); }
  };
  return <VaultModal title={t(browsing ? "nb.folder" : "nb.createVault")} busy={busy}>
    <form onSubmit={e => { e.preventDefault(); if (!busy && !browsing) void submit(); }}>
      {!browsing && <fieldset disabled={busy}>
        <label>{t("nb.name")}<input className="input" required autoFocus value={name} onChange={e => { setName(e.target.value); setError(""); }} /></label>
        <label>{t("nb.folder")}<div className="nb-inline"><input className="input" required value={parent} onChange={e => { setParent(e.target.value); setError(""); }} /><button className="btn" type="button" onClick={() => void chooseParent()}>{t("nb.browse")}</button></div></label>
        {parent && name && <p className="nb-vault-target">{joinPath(parent, name)}</p>}
      </fieldset>}
      {browsing && <><ServerBrowserView browser={browser} /><div className="nb-vault-pick"><button type="button" className="btn" onClick={() => setBrowsing(false)}>{t("common.cancel")}</button><button type="button" className="btn" disabled={!browser.selectedDir} onClick={() => { setParent(browser.selectedDir); setBrowsing(false); }}>{t("dir.choose")}</button></div></>}
      {!browsing && error && <NotebookError error={error} />}
      {!browsing && <footer><button className="btn" type="button" disabled={busy} onClick={dismissVaultAction}>{t("common.cancel")}</button><button className="btn btn-primary" disabled={busy || browsing || !parent || !name}>{t(busy ? "common.loading" : "nb.createVault")}</button></footer>}
    </form>
  </VaultModal>;
}
