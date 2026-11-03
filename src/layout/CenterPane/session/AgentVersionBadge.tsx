import { useEffect, useState } from "react";
import { useT } from "../../../i18n";
import { agentVersion, type AgentVersion } from "../../../ipc/agentVersion";
import { isShareSurface } from "../../../ipc/shareBase";
import { onTransportReconnect } from "../../../ipc/transport";
import { useTermStore } from "../../../store/termStore";
import type { Session } from "../../../types";
import { AgentUpdateDialog } from "./AgentUpdateDialog";

/** The server owns executable selection, release sources, and version comparison. */
export function AgentVersionBadge({ session, hidden }: { session: Session; hidden: boolean }) {
  const t = useT();
  const globalPath = useTermStore(s => s.agentDefaults[session.kind]?.path);
  const startedAt = useTermStore(s => s.runtimes[session.id]?.startedAt);
  const identity = JSON.stringify([session.id, session.kind, session.agentPath, globalPath, startedAt]);
  const [result, setResult] = useState<{ identity: string; data: AgentVersion | null; failed: boolean }>();
  // Captured when the dialog opens: a finished update clears updateAvailable, and the dialog must stay to report it.
  const [updating, setUpdating] = useState<{ from: string; to: string; command: string; docsUrl: string | null; running: boolean } | null>(null);
  // Bumped after an update finishes so the installed version is read again.
  const [generation, setGeneration] = useState(0);

  useEffect(() => {
    if (hidden) return;
    let alive = true;
    let pending = false;
    const refresh = async () => {
      if (pending || document.visibilityState === "hidden") return;
      pending = true;
      try {
        const data = await agentVersion(session.id);
        if (!alive) return;
        setResult({ identity, data, failed: false });
        const checked = await agentVersion(session.id, true);
        if (alive) setResult({ identity, data: checked, failed: false });
      } catch {
        if (alive) setResult(previous => ({ identity, data: previous?.identity === identity ? previous.data : null, failed: true }));
      } finally {
        pending = false;
      }
    };
    void refresh();
    const timer = window.setInterval(() => void refresh(), 5 * 60 * 1000);
    const visible = () => { if (document.visibilityState === "visible") void refresh(); };
    window.addEventListener("focus", visible);
    document.addEventListener("visibilitychange", visible);
    const unlisten = onTransportReconnect(() => void refresh());
    return () => {
      alive = false;
      window.clearInterval(timer);
      window.removeEventListener("focus", visible);
      document.removeEventListener("visibilitychange", visible);
      unlisten();
    };
  }, [identity, session.id, hidden, generation]);

  const current = result?.identity === identity ? result : undefined;
  const data = current?.data;
  const failed = current?.failed || data?.checkStatus === "failed";
  const updateVersion = data?.version && data.updateAvailable ? data.latestVersion : null;
  const updateUrl = updateVersion ? data?.docsUrl : null;
  // Visitors on a shared surface may not run commands on the host; they keep the instructions link.
  const updateCommand = updateVersion && !isShareSurface ? data?.updateCommand : null;
  const version = data?.version ? `v${data.version}` : t(current ? "agentVersion.unavailable" : "agentVersion.loading");
  const versionTitle = data?.version
    ? t(data.running ? "agentVersion.running" : "agentVersion.installed", data.version)
    : t("agentVersion.unavailable");
  const title = [versionTitle, failed ? t("agentVersion.checkFailed") : null].filter(Boolean).join("\n");

  // The available update stays visible beside the version. It opens the update dialog when the agent can update
  // itself, falls back to the official instructions, and is plain text when neither exists.
  return <span className="sv-agent-version" role="status" aria-label={t("agentVersion.label")}>
    <span className="sv-agent-version-current" title={title}>{version}</span>
    {updateVersion && (updateCommand ? <button
      type="button" className="sv-agent-version-update" onClick={() => setUpdating({
        from: data!.version!, to: updateVersion, command: updateCommand, docsUrl: data!.docsUrl, running: data!.running,
      })}
      title={t("agentVersion.upgradeAction", updateVersion)}
    >{t("agentVersion.update", updateVersion)}</button> : updateUrl ? <a
      className="sv-agent-version-update" href={updateUrl} target="_blank" rel="noopener noreferrer"
      title={t("agentVersion.upgrade", updateVersion)}
    >{t("agentVersion.update", updateVersion)}</a> : <span className="sv-agent-version-update">{t("agentVersion.update", updateVersion)}</span>)}
    {updating && <AgentUpdateDialog
      session={session} from={updating.from} to={updating.to} command={updating.command}
      docsUrl={updating.docsUrl} agentRunning={updating.running}
      onClose={() => setUpdating(null)} onFinished={() => setGeneration(g => g + 1)}
    />}
  </span>;
}
