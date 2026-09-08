import { sharingText } from "../../sharing/copy";
import { getLocale, useT } from "../../i18n";
import { useEffect, useState } from "react";
import { invoke } from "../../ipc/transport";
import { copyText } from "../../ipc/info";
import { platform } from "../../platform";
import "./public-sharing.css";

interface Options { options: { scopes: string[]; accessModes: string[]; maxExpiryDays: number }; projects: { id: string; name: string }[]; sessions: { id: string; name: string }[] }

export function PublicSharingPanel() {
  useT();
  const t = (message: string) => sharingText(message, getLocale());
  const [linked, setLinked] = useState(false);
  const [linking, setLinking] = useState(false);
  const [publicKey, setPublicKey] = useState("");
  const [links, setLinks] = useState<{ id: string; scope: string; url: string }[]>([]);
  const [options, setOptions] = useState<Options | null>(null);
  const [scope, setScope] = useState("");
  const [target, setTarget] = useState("");
  const [access, setAccess] = useState("");
  const [accounts, setAccounts] = useState("");
  const [password, setPassword] = useState("");
  const [expires, setExpires] = useState("");
  const [url, setUrl] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  async function refresh() {
    const status = await invoke<{ linked: boolean }>("public_account_status");
    setLinked(status.linked);
    if (status.linked) {
      setOptions(await invoke<Options>("public_share_options"));
      setLinks(await invoke("public_share_links"));
    }
  }
  useEffect(() => { void refresh().catch(() => setError(t("Public sharing service unavailable."))); }, []);
  async function action(job: () => Promise<void>) {
    setBusy(true); setError("");
    try { await job(); } catch { setError(t("The request failed. Check the account service and try again.")); }
    finally { setBusy(false); }
  }
  return <section className="public-sharing">
    <h3>{t("Public conversation sharing")}</h3>
    <p>{t("Share AI conversations through velaterm.com. Direct terminal access is disabled; AI retains the host user's file and command permissions.")}</p>
    {error && <p role="alert">{error}</p>}
    {!linked ? <>
      <button disabled={busy} onClick={() => void action(async () => {
        const result = await invoke<{ url: string; publicKey: string }>("public_account_link", { name: "VelaTerm" });
        setPublicKey(result.publicKey);
        await platform.opener.openExternal(result.url); setLinking(true);
      })}>{t("Sign in and link this device")}</button>
      {linking && <p><code>{publicKey}</code></p>}
      {linking && <button disabled={busy} onClick={() => void action(async () => {
        const result = await invoke<{ linked: boolean }>("public_account_poll");
        if (result.linked) { setLinking(false); await refresh(); } else setError(t("Approve the device in your browser, then check again."));
      })}>{t("Check approval")}</button>}
    </> : <>
      <a href="https://velaterm.com/account" target="_blank" rel="noreferrer" onClick={e => { e.preventDefault(); void platform.opener.openExternal(e.currentTarget.href); }}>{t("Manage account and revoke links")}</a>
      {options && <form onSubmit={e => { e.preventDefault(); void action(async () => {
        const result = await invoke<{ url: string }>("public_share_create", {
          scope, targetId: scope === "machine" ? null : target, accessMode: access,
          accountIds: access === "accounts" ? accounts.split(/[\s,]+/).filter(Boolean) : [],
          password: access === "link" ? password : "", expiresAt: new Date(expires).toISOString(),
        }); setUrl(result.url); setPassword(""); await refresh();
      }); }}>
        <label>{t("Scope")}<select required value={scope} onChange={e => { setScope(e.target.value); setTarget(""); }}><option value="">{t("Select…")}</option>{options.options.scopes.map(s => <option key={s} value={s}>{t(s)}</option>)}</select></label>
        {scope && scope !== "machine" && <label>{t("Shared item")}<select required value={target} onChange={e => setTarget(e.target.value)}><option value="">{t("Select…")}</option>{(scope === "project" ? options.projects : options.sessions).map(s => <option key={s.id} value={s.id}>{s.name}</option>)}</select></label>}
        <label>{t("Access")}<select required value={access} onChange={e => setAccess(e.target.value)}><option value="">{t("Select…")}</option>{options.options.accessModes.map(s => <option key={s} value={s}>{t(s)}</option>)}</select></label>
        {access === "accounts" && <label>{t("Account IDs")}<input required value={accounts} onChange={e => setAccounts(e.target.value)} /></label>}
        {access === "link" && <label>{t("Optional password")}<input type="password" minLength={8} maxLength={256} value={password} onChange={e => setPassword(e.target.value)} /></label>}
        <label>{t("Expires")}<input required type="datetime-local" value={expires} onChange={e => setExpires(e.target.value)} /></label>
        <button disabled={busy}>{t("Create sharing link")}</button>
      </form>}
      {url && <div><input aria-label={t("Sharing link")} value={url} readOnly style={{ width: "100%" }} /><button onClick={() => void copyText(url)}>{t("Copy link")}</button></div>}
      {links.map(link => <div key={link.id}><span>{t(link.scope)}</span>{" "}<button onClick={() => void copyText(link.url)}>{t("Copy link")}</button></div>)}
    </>}
  </section>;
}
