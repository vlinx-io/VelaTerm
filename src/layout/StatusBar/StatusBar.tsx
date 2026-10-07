//! Bottom status bar (Vlinx style, 24px). Left: session count, current session type, branch and
//! worktree marker, per-session permissions, notifications, Feedback, and Account. Right: global aggregates that
//! do not duplicate the per-session Info panel—working/waiting/replied counts linked to the sidebar
//! filter, background keep-alive tab count with a popover, and remote-access status.

import { useAgentPermissions, savePermissionDefault } from "../../hooks/useAgentPermissions";
import { permissionLabelKey } from "../../components/AgentPermissionSelect";
import { useSessionPermissionState, type SessionPermissionState } from "../../hooks/useSessionPermissionState";
import { currentPermissionLabel, effectivePermission, pendingPermissionLabel, PermissionStateDetails } from "../../components/PermissionStateDetails";
import { useEffect, useRef, useState } from "react";
import Icons from "../../components/Icons";
import { SELECT_PANEL } from "../../components/Select";
import { SessionStatusBadge } from "../../components/SessionStatusBadge";
import { useGitBranchInfo } from "../../hooks/useGitBranch";
import { getLocale, t, useT } from "../../i18n";
import { SharingLink } from "../../sharing/navigation";
import { sharingText } from "../../sharing/copy";
import { invoke, isTauri } from "../../ipc/transport";
import {
  getEffectiveNotifyPermission,
  requestEffectiveNotifyPermission,
} from "../../notify";
import { openUpdateModal, useUpdateState } from "../../ipc/updater";
import { checkVelaSkills, openVelaSkillsModal, useVelaSkillsState } from "../../ipc/velaSkills";
import { isShareSurface } from "../../ipc/shareBase";
import { webServerStatus, type WebServerStatus } from "../../ipc/webServer";
import { env, platform } from "../../platform";
import { useTermStore } from "../../store/termStore";
import { effectivePermissionMode } from "../../store/settings";
import {
  countByAgentState,
  type SessionKind,
  supportsPermissionToggle,
} from "../../types";
import { collectSessionIds } from "../CenterPane/paneTree";

const FEEDBACK_URL = "https://velaterm.com/feedback";

/** Localized display name for a session type; brand names remain untranslated. */
function kindLabel(kind: SessionKind): string {
  if (kind === "terminal") return t("kind.terminal");
  if (kind === "claude") return "Claude";
  if (kind === "codex") return "Codex";
  if (kind === "opencode") return "OpenCode";
  if (kind === "copilot") return "Copilot";
  if (kind === "cursor") return "Cursor";
  if (kind === "antigravity") return "Antigravity";
  if (kind === "cline") return "Cline";
  if (kind === "pi") return "Pi";
  if (kind === "omp") return "OMP";
  if (kind === "crush") return "Crush";
  if (kind === "kimi") return "Kimi Code";
  if (kind === "kiro") return "Kiro";
  if (kind === "grok") return "Grok Build";
  if (kind === "zoo") return "Zoo Code";
  return t("kind.browser");
}

/**
 * Agent-state aggregate on the right. Clicking a state replaces the primary sidebar's current status
 * selection; clicking the sole active state again clears it. Working and waiting icons use the same
 * pulse animation as sidebar status dots. Labels reuse the `tree.filter*` keys so wording stays
 * synchronized across both controls.
 */
const STATUS_SEGS = [
  {
    st: "working",
    icon: Icons.bot,
    pulse: true,
    color: "var(--status-working)",
    labelKey: "tree.filterWorking",
  },
  {
    st: "asking",
    icon: Icons.bell,
    pulse: true,
    color: "var(--status-asking)",
    labelKey: "tree.filterAsking",
  },
  {
    st: "waiting",
    icon: Icons.check,
    pulse: false,
    color: "var(--status-waiting)",
    labelKey: "tree.filterWaiting",
  },
  {
    st: "background",
    icon: Icons.layers,
    pulse: false,
    color: "var(--status-background)",
    labelKey: "tree.filterBackground",
  },
] as const;

/** Agent-state aggregate isolated to absorb frequent runtime updates and the full `countByAgentState`
 * scan. Agent status changes then rerender only this small section instead of recomputing and
 * rerendering the entire status bar in O(N) session time. */
function StatusSegs() {
  const t = useT();
  const sessions = useTermStore((s) => s.sessions);
  const runtimes = useTermStore((s) => s.runtimes);
  const notifications = useTermStore((s) => s.notifications);
  const statusFilter = useTermStore((s) => s.statusFilter);
  const setStatusFilter = useTermStore((s) => s.setStatusFilter);
  const counts = countByAgentState(sessions, runtimes, notifications);
  return (
    <>
      {STATUS_SEGS.map(({ st, icon: Icon, pulse, color, labelKey }) => {
        const n = counts[st];
        const on = statusFilter?.includes(st) ?? false;
        // Hide zero counts unless that filter is active, in which case keep an entry for clearing it.
        if (n === 0 && !on) return null;
        return (
          <span
            key={st}
            className={on ? "seg btn on" : "seg btn"}
            style={{ color }}
            title={t("statusbar.filterTooltip", t(labelKey))}
            onClick={() => setStatusFilter(st)}
          >
            <span
              className={pulse && n > 0 ? "vlx-status-pulse" : undefined}
              style={{ display: "inline-flex" }}
            >
              <Icon size={11} />
            </span>
            {`${n} ${t(labelKey)}`}
          </span>
        );
      })}
    </>
  );
}

export function StatusBar() {
  const t = useT();
  const sessions = useTermStore((s) => s.sessions);
  const ephemeralSessions = useTermStore((s) => s.ephemeralSessions);
  const projects = useTermStore((s) => s.projects);
  const activeSessionId = useTermStore((s) => s.activeSessionId);
  const liveTabs = useTermStore((s) => s.liveTabs);
  const liveEvictNotice = useTermStore((s) => s.liveEvictNotice);
  const clearLiveEvictNotice = useTermStore((s) => s.clearLiveEvictNotice);

  // Show background-tab eviction notices briefly; a new timestamp restarts the timer.
  useEffect(() => {
    if (!liveEvictNotice) return;
    const timer = setTimeout(clearLiveEvictNotice, 8000);
    return () => clearTimeout(timer);
  }, [liveEvictNotice, clearLiveEvictNotice]);

  const session = activeSessionId
    ? (sessions.find((s) => s.id === activeSessionId) ??
      ephemeralSessions[activeSessionId])
    : undefined;
  const project = session
    ? projects.find((p) => p.id === session.projectId)
    : undefined;
  const cwd = session?.cwd || project?.rootPath || null;
  const { branch, isWorktree, worktreePath } = useGitBranchInfo(cwd);
  // Use the worktree directory's final path segment as the branch marker tooltip.
  const worktreeDir = worktreePath
    ? (worktreePath.split("/").filter(Boolean).pop() ?? worktreePath)
    : null;

  // Tauri and Electron expose remote-access status; browser clients are already remote, so omit it.
  const [web, setWeb] = useState<WebServerStatus | null>(null);
  useEffect(() => {
    if (!isTauri && !env.isElectron) return;
    let cancelled = false;
    const poll = () =>
      webServerStatus()
        .then((s) => !cancelled && setWeb(s))
        .catch(() => !cancelled && setWeb(null));
    poll();
    const t = setInterval(poll, 5000);
    return () => {
      cancelled = true;
      clearInterval(t);
    };
  }, []);

  return (
    <div className="statusbar">
      <span className="seg">{t("statusbar.sessions", sessions.length)}</span>
      <span className="seg">{session ? kindLabel(session.kind) : t("status.idle")}</span>
      <span className="seg">
        <Icons.branch size={11} />
        <span className="accent">{branch ?? "—"}</span>
        {isWorktree && (
          <span
            title={worktreeDir ?? undefined}
            style={{ display: "inline-flex", marginLeft: 1, color: "var(--text-dim)" }}
          >
            <Icons.folder size={10} />
          </span>
        )}
      </span>
      <PermissionSeg />
      <NotifySeg />
      <FeedbackSeg />
      <AccountSeg />
      <span className="sp" />
      <StatusSegs />
      {liveTabs.length > 0 && <BackgroundTabsSeg />}
      {liveEvictNotice && (
        <span className="seg">
          <span className="accent">{t("statusbar.bgEvicted", liveEvictNotice.label)}</span>
        </span>
      )}
      {web?.running && (
        <span className="seg web-on" title={t("statusbar.webTooltip", web.url ?? "")}>
          <Icons.globe size={11} />:{web.port}
        </span>
      )}
      <VelaSkillsSeg />
      <UpdateSeg />
    </div>
  );
}

/** Opens the feedback page in the system browser. */
function FeedbackSeg() {
  const t = useT();
  return (
    <button
      type="button"
      className="seg btn"
      title={t("titlebar.feedback")}
      aria-label={t("titlebar.feedback")}
      onClick={() => {
        void platform.opener.openExternal(FEEDBACK_URL).catch(() => {});
      }}
    >
      <Icons.feedback size={11} />
    </button>
  );
}

/** Account link state follows changes reported by the local account relay. */
function AccountSeg() {
  const t = useT();
  const [linked, setLinked] = useState(false);
  useEffect(() => {
    if (env.isBrowser) return;
    let alive = true;
    const refresh = () => {
      invoke<{ linked: boolean }>("public_account_status")
        .then((status) => {
          if (alive) setLinked(status.linked);
        })
        .catch(() => {
          // Keep the last known state while the account service is unreachable.
        });
    };
    refresh();
    window.addEventListener("public-account-changed", refresh);
    return () => {
      alive = false;
      window.removeEventListener("public-account-changed", refresh);
    };
  }, []);
  if (env.isBrowser) return null;
  const label = `${sharingText("Account", getLocale())} · ${t("common.experimental")}`;
  return (
    <SharingLink
      className={"seg btn" + (linked ? " account-on" : "")}
      values={{ publicAccount: "1" }}
      title={label}
      aria-label={label}
    >
      <Icons.account size={11} />
    </SharingLink>
  );
}

/**
 * New-version indicator. A silent startup check only highlights this section rather than interrupting
 * the user; clicking it opens UpdateModal to review release notes and choose whether to install.
 *
 * The modal may be closed during download because progress remains visible here. Installation changes
 * the label to "restart to finish," while failures turn it red. Skipping the version removes the
 * section. Browser and remote clients do not self-update, so their null prompt keeps it hidden.
 */
function UpdateSeg() {
  const t = useT();
  const { prompt, stage } = useUpdateState();
  if (!prompt) return null;

  let Icon = Icons.download;
  let color = "var(--accent)";
  let label: string;
  switch (stage.kind) {
    case "downloading":
      // Without Content-Length a percentage is unavailable, so fall back to the generic installing text.
      label =
        stage.total > 0
          ? t(
              "statusbar.updateDownloading",
              Math.min(100, Math.round((stage.received / stage.total) * 100)),
            )
          : t("statusbar.updateInstalling");
      break;
    case "installing":
      label = t("statusbar.updateInstalling");
      break;
    case "ready":
      Icon = Icons.restart;
      label = t("statusbar.updateReady");
      break;
    case "error":
      color = "var(--status-asking)";
      label = t("statusbar.updateFailed");
      break;
    default:
      label = t("statusbar.updateAvailable", prompt.version);
  }

  return (
    <span
      className="seg btn"
      style={{ color }}
      title={t("statusbar.updateTooltip")}
      onClick={openUpdateModal}
    >
      <Icon size={11} />
      {label}
    </span>
  );
}

/**
 * Vela Skills prompt. A startup check only shows this section when the skills are missing; clicking it
 * opens VelaSkillsModal to explain them. Installing or choosing "Don't remind me again" removes it.
 * Share surfaces never show it because guests must not change the host's agent setup.
 */
function VelaSkillsSeg() {
  const t = useT();
  const { installed, dismissed } = useVelaSkillsState();
  // Check once at mount; a dismissed prompt needs no check.
  useEffect(() => {
    if (!isShareSurface && !dismissed) void checkVelaSkills();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  if (isShareSurface || dismissed || installed !== false) return null;
  return (
    <span
      className="seg btn"
      style={{ color: "var(--accent)" }}
      title={t("statusbar.updateTooltip")}
      onClick={openVelaSkillsModal}
    >
      <Icons.sparkle size={11} />
      {t("statusbar.skillsAvailable")}
    </span>
  );
}

/** Persistent system-notification toggle: bell means on, crossed-out bell means off. It shares the
 * settings-page preference and requests OS permission when enabled if permission is still pending. */
function NotifySeg() {
  const t = useT();
  const notifyEnabled = useTermStore((s) => s.notifyEnabled);
  const toggleNotify = useTermStore((s) => s.toggleNotify);
  const Icon = notifyEnabled ? Icons.bell : Icons.bellOff;
  const state = notifyEnabled ? t("common.on") : t("common.off");
  return (
    <span
      className="seg btn"
      style={{ color: notifyEnabled ? undefined : "var(--text-dim)" }}
      title={`${t("settings.notify")} · ${state}`}
      onClick={() => {
        const wasOn = notifyEnabled;
        toggleNotify();
        // Request once when enabled without permission; the OS prompts only if no decision exists.
        if (!wasOn) {
          void getEffectiveNotifyPermission().then(async (p) => {
            // If previously denied, guide the user to system settings. Otherwise show the native
            // permission prompt first and display guidance only if it is denied.
            if (p === "denied") {
              useTermStore.getState().setNotifyGuideOpen(true);
            } else if (p === "default") {
              const r = await requestEffectiveNotifyPermission();
              if (r === "denied")
                useTermStore.getState().setNotifyGuideOpen(true);
            }
          });
        }
      }}
    >
      <Icon size={11} />
      {notifyEnabled ? t("statusbar.notifyOn") : t("statusbar.notifyOff")}
    </span>
  );
}

/** "Background x/32" indicator and upward-opening keep-alive list. Click a name to restore its whole
 * tree or the × button to terminate it. */
function BackgroundTabsSeg() {
  const t = useT();
  const liveTabs = useTermStore((s) => s.liveTabs);
  const maxLiveTabs = useTermStore((s) => s.maxLiveTabs);
  const paneTrees = useTermStore((s) => s.paneTrees);
  const sessions = useTermStore((s) => s.sessions);
  const openSession = useTermStore((s) => s.openSession);
  const closeLiveTab = useTermStore((s) => s.closeLiveTab);
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLSpanElement | null>(null);

  // Dismiss the list when clicking outside it.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, [open]);

  if (liveTabs.length === 0) return null;

  return (
    <span
      ref={rootRef}
      className="seg btn"
      style={{ position: "relative" }}
      title={t("statusbar.bgTooltip", maxLiveTabs)}
      onClick={() => setOpen((v) => !v)}
    >
      {t("statusbar.bgCount", liveTabs.length, maxLiveTabs)}
      {open && (
        <div
          style={{
            ...SELECT_PANEL,
            top: undefined, // Opens upward: the status bar sits at the bottom of the window.
            bottom: "100%",
            right: 0,
            marginBottom: 6,
            minWidth: 280,
            maxWidth: 400,
            cursor: "default",
          }}
          onClick={(e) => e.stopPropagation()}
        >
          {/* Displayed newest first, so the most recently backgrounded session is at the top, matching
              the instinct to switch back to what you just used. Only a copy is reversed at render time:
              liveTabs in the store keeps its original backgrounding order so eviction (pickEvictTab) can
              still drop the oldest idle tab first. */}
          {[...liveTabs].reverse().map((tabId) => {
            const tree = paneTrees[tabId];
            const ids = tree ? collectSessionIds(tree) : [];
            const named = ids
              .map((sid) => sessions.find((s) => s.id === sid))
              .filter((s): s is NonNullable<typeof s> => !!s);
            const label =
              named.map((session) => session.name).join(" ⫽ ") ||
              t("tab.scratchFallback");
            // Use the first named session as the entry point; openSession restores the entire tree.
            const switchTo = named[0]?.id;
            return (
              <div
                key={tabId}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 6,
                  padding: "5px 8px",
                  borderRadius: 6,
                  cursor: switchTo ? "pointer" : "default",
                  fontSize: 12,
                  color: "var(--text)",
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = "var(--bg-hover)";
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = "transparent";
                }}
                onClick={() => {
                  if (!switchTo) return;
                  setOpen(false);
                  openSession(switchTo);
                }}
              >
                <span
                  style={{
                    flex: 1,
                    minWidth: 0,
                    display: "flex",
                    flexDirection: "column",
                    gap: 3,
                  }}
                  title={label}
                >
                  {named.length > 0 ? (
                    named.map((session) => (
                      <span
                        key={session.id}
                        style={{
                          display: "flex",
                          alignItems: "center",
                          gap: 10,
                          minWidth: 0,
                        }}
                      >
                        <span
                          style={{
                            flex: 1,
                            minWidth: 0,
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                          }}
                        >
                          {session.name}
                        </span>
                        <SessionStatusBadge sessionId={session.id} />
                      </span>
                    ))
                  ) : (
                    <span>{t("tab.scratchFallback")}</span>
                  )}
                </span>
                <span
                  title={t("tab.killBgTab")}
                  style={{ display: "grid", color: "var(--text-dim)", flex: "none" }}
                  onClick={(e) => {
                    e.stopPropagation();
                    closeLiveTab(tabId);
                  }}
                >
                  <Icons.x size={12} />
                </span>
              </div>
            );
          })}
        </div>
      )}
    </span>
  );
}

/** Per-session terminal permissions use the same choices as settings and chat. Saving a default is
 * explicit; running terminals still require confirmation before restarting to apply a saved mode. */
export function PermissionSeg() {
  const t = useT();
  const activeSessionId = useTermStore((s) => s.activeSessionId);
  const sessions = useTermStore((s) => s.sessions);
  // Subscribe only to this session's running flag so other runtime updates do not rerender this section.
  const activeRunning = useTermStore((s) =>
    s.activeSessionId ? s.runtimes[s.activeSessionId]?.status === "running" : false,
  );
  const updateSession = useTermStore((s) => s.updateSession);
  const restartSession = useTermStore((s) => s.restartSession);
  const [open, setOpen] = useState(false);
  // Two popover stages: choose a mode, then confirm a restart if the session is running.
  const [step, setStep] = useState<"mode" | "restart">("mode");
  const rootRef = useRef<HTMLSpanElement | null>(null);

  // Clicking outside closes the popover and exits restart confirmation.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) {
        setOpen(false);
        setStep("mode");
      }
    };
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, [open]);

  // Show only for persisted agent sessions that support permission switching, not temporary drafts.
  const session = sessions.find((s) => s.id === activeSessionId);
  const agentDefaults = useTermStore((s) => s.agentDefaults);
  const permissionMode = session ? effectivePermissionMode(session, agentDefaults) : null;
  const permissionCatalog = useAgentPermissions(session?.kind ?? "terminal", permissionMode);
  const permissionState = useSessionPermissionState(session?.engine === "chat" ? undefined : session?.id,
    `${permissionMode}:${activeRunning}`);
  const [keepPermission, setKeepPermission] = useState(false);
  const [permissionError, setPermissionError] = useState<string>();
  useEffect(() => {
    setOpen(false); setStep("mode"); setKeepPermission(false); setPermissionError(undefined);
  }, [activeSessionId]);
  if (!session || !supportsPermissionToggle(session.kind)) return null;
  // Chat sessions expose the same runtime facts beside their composer.
  if (session.engine === "chat") return null;

  const isSkip = ["skip", "bypassPermissions", "full-access"].includes(permissionMode ?? "");
  const catalogued = ["claude", "codex", "opencode"].includes(session.kind);

  const close = () => {
    setOpen(false);
    setStep("mode");
  };

  // Persist only this session's permissionMode. Because update_session replaces the full record,
  // pass existing fields too or values such as the session name would be cleared.
  const chooseMode = async (mode: string) => {
    setPermissionError(undefined);
    await updateSession(session.id, {
      name: session.name,
      shell: session.shell ?? null,
      cwd: session.cwd ?? null,
      initCmd: session.initCmd ?? null,
      agentArgs: session.agentArgs ?? null,
      permissionMode: mode,
    });
    const saved = await invoke<SessionPermissionState>("session_permission_state", { sessionId: session.id });
    const needsRestart = saved.activation === "restart";
    if (needsRestart) setStep("restart");
    if (keepPermission) await savePermissionDefault(session.kind, mode);
    if (!needsRestart) close();
  };

  const doRestart = async () => {
    await restartSession(session.id);
    close();
  };

  const menuOption = (
    active: boolean,
    danger: boolean,
    label: string,
    onClick: () => void,
  ) => (
    <button type="button"
      onClick={onClick}
      style={{
        width: "100%", border: 0, background: "transparent", textAlign: "left",
        display: "flex",
        alignItems: "center",
        gap: 8,
        padding: "6px 10px",
        borderRadius: 6,
        cursor: "pointer",
        fontSize: 12,
        color: danger ? "var(--status-asking)" : "var(--text)",
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.background = "var(--bg-hover)";
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.background = "transparent";
      }}
    >
      <span style={{ width: 14, display: "inline-flex", flex: "none" }}>
        {active ? <Icons.check size={12} /> : null}
      </span>
      <span>{label}</span>
    </button>
  );

  return (
    <span
      ref={rootRef}
      className={["skip", "bypassPermissions", "full-access"].includes(effectivePermission(permissionState?.value) ?? "") ? "seg btn on" : "seg btn"}
      style={{
        position: "relative",
      }}
      title={t(catalogued ? "chat.modeTooltip" : "statusbar.permTooltip")}
      onClick={() => {
        if (!open) { setKeepPermission(false); setPermissionError(undefined); }
        setOpen(v => !v);
      }}
    >
      <Icons.lock size={11} />
      {currentPermissionLabel(permissionState?.value)}
      {permissionState?.value?.pending && <span style={{ marginLeft: 6 }}>{pendingPermissionLabel(permissionState.value)}</span>}
      {open && (
        <div
          style={{
            ...SELECT_PANEL,
            top: undefined, // Opens upward, as above.
            bottom: "100%",
            left: 0,
            marginBottom: 6,
            minWidth: 240,
            maxWidth: 340,
            maxHeight: undefined,
            padding: 6,
            cursor: "default",
            color: "var(--text)",
          }}
          onClick={(e) => e.stopPropagation()}
        >
          {permissionError && <div role="alert">{permissionError}</div>}
          <div style={{ padding: "6px 10px", marginBottom: 6 }}>
            <PermissionStateDetails state={permissionState?.value} error={permissionState?.error} />
          </div>
          {step === "mode" && (
            <>
              <div
                style={{
                  fontSize: 11,
                  color: "var(--text-dim)",
                  padding: "2px 10px 6px",
                }}
              >
                {t("statusbar.permMenuTitle")}
              </div>
              {catalogued ? <>
                {permissionCatalog?.catalog?.modes.map(mode => <span key={mode}>
                  {menuOption(permissionCatalog.catalog?.selected === mode, mode === "bypassPermissions" || mode === "full-access",
                    t(permissionLabelKey(mode)), () => void chooseMode(mode).catch(error => setPermissionError(String(error))))}
                </span>)}
                {permissionCatalog?.catalog && <label style={{ display: "flex", gap: 8, padding: "6px 10px" }}>
                  <input type="checkbox" checked={keepPermission} onChange={e => setKeepPermission(e.target.checked)} />
                  {t("chat.keepChoice")}
                </label>}
                {permissionCatalog?.error && <div role="alert">{permissionCatalog.error}</div>}
              </> : <>
                {menuOption(!isSkip, false, t("statusbar.permOptAsk"), () => void chooseMode("default").catch(error => setPermissionError(String(error))))}
                {menuOption(isSkip, true, t("tree.permissionSkipLabel"), () => void chooseMode("skip").catch(error => setPermissionError(String(error))))}
              </>}
              <div
                style={{
                  fontSize: 11,
                  color: "var(--text-dim)",
                  padding: "6px 10px 2px",
                  lineHeight: 1.5,
                }}
              >
                {!catalogued && t("statusbar.permScopeHint")}
              </div>
            </>
          )}
          {step === "restart" && (
            <>
              <div
                style={{
                  fontSize: 12,
                  padding: "4px 10px 10px",
                  lineHeight: 1.6,
                }}
              >
                {t("statusbar.permRestartMsg")}
              </div>
              <div
                style={{
                  display: "flex",
                  justifyContent: "flex-end",
                  gap: 8,
                  padding: "0 4px 2px",
                }}
              >
                <button
                  onClick={close}
                  style={{
                    fontSize: 11,
                    padding: "4px 12px",
                    borderRadius: 5,
                    border: "1px solid var(--border)",
                    background: "transparent",
                    color: "var(--text)",
                    cursor: "pointer",
                  }}
                >
                  {t("statusbar.permRestartLater")}
                </button>
                <button
                  onClick={() => void doRestart().catch(error => setPermissionError(String(error)))}
                  style={{
                    fontSize: 11,
                    padding: "4px 12px",
                    borderRadius: 5,
                    border: "1px solid var(--accent)",
                    background: "var(--accent)",
                    color: "var(--text-on-accent)",
                    cursor: "pointer",
                    display: "inline-flex",
                    alignItems: "center",
                    gap: 5,
                  }}
                >
                  <Icons.restart size={12} />
                  {t("statusbar.permRestartNow")}
                </button>
              </div>
            </>
          )}
        </div>
      )}
    </span>
  );
}
