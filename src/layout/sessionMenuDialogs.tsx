//! Dialogs supporting the session menu: deletion, session details, Codex notices, group creation, custom
//! agent creation, and session restoration. Extracted from sessionMenu.tsx; useSessionMenu renders them
//! on demand, while shared helpers such as kindLabel, worktreeLabel, and InfoRow live in sessionMenuShared.

import { useEffect, useState } from "react";
import { normalizeArgDashes } from "../args";
import { Backdrop } from "../components/Backdrop";
import Icons from "../components/Icons";
import { useGitBranchInfo } from "../hooks/useGitBranch";
import { useSuspendNativeViews } from "../hooks/nativeViewSuspend";
import { dateLocale, useT } from "../i18n";
import {
  type CommitInfo,
  gitRecentCommits,
  listWorktrees,
  type WorktreeEntry,
} from "../ipc/commands";
import { useTermStore } from "../store/termStore";
import {
  effectiveStatus,
  type NodeKind,
  type SessionKind,
  supportsPermissionToggle,
} from "../types";
import {
  AGENT_ARGS_KINDS,
  InfoRow,
  kindLabel,
  RESUMABLE_KINDS,
  statusLabel,
  type WorktreeChoice,
  worktreeLabel,
} from "./sessionMenuShared";
import { kindIconEl } from "./sessionViewers/sessionMeta";

export function ConfirmDelete({
  name,
  kind,
  batchCount,
  worktreePaths = [],
  onConfirm,
  onCancel,
}: {
  name: string;
  kind: NodeKind;
  batchCount?: number;
  worktreePaths?: string[];
  onConfirm: (removeWorktree: boolean) => void;
  onCancel: () => void;
}) {
  const t = useT();
  const isBatch = batchCount != null;
  const hasWt = worktreePaths.length > 0;
  const [removeWt, setRemoveWt] = useState(false);
  const title = isBatch
    ? t("tree.batchDeleteTitle")
    : kind === "project"
      ? t("tree.deleteProjectTitle")
      : kind === "group"
        ? t("tree.deleteGroupTitle")
        : t("tree.deleteSessionTitle");
  const body = isBatch
    ? t("tree.batchDeleteBody", batchCount)
    : kind === "project"
      ? t("tree.deleteProjectBody", name)
      : kind === "group"
        ? t("tree.deleteGroupBody", name)
        : t("tree.deleteSessionBody", name);
  return (
    <Backdrop onClose={onCancel}>
      <div
        style={{
          width: 400,
          maxWidth: "calc(100vw - 32px)",
          boxSizing: "border-box",
          background: "var(--bg-panel)",
          border: "1px solid var(--border)",
          borderRadius: 10,
          padding: 18,
          boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)", marginBottom: 10 }}>
          {title}
        </div>
        <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.6, marginBottom: 18 }}>
          {body}
        </div>
        {hasWt && (
          <label
            style={{
              display: "grid",
              gridTemplateColumns: "16px minmax(0, 1fr)",
              alignItems: "start",
              gap: 10,
              padding: "10px 12px",
              fontSize: 13,
              color: "var(--text-secondary)",
              lineHeight: 1.5,
              marginBottom: 16,
              background: "var(--bg-elevated)",
              border: "1px solid var(--border)",
              borderRadius: 8,
              cursor: "pointer",
            }}
          >
            <input
              type="checkbox"
              checked={removeWt}
              onChange={(e) => setRemoveWt(e.target.checked)}
              style={{
                width: 16,
                height: 16,
                margin: "2px 0 0",
                accentColor: "var(--accent)",
                cursor: "pointer",
              }}
            />
            <span>{t("tree.deleteWorktrees", worktreePaths.length)}</span>
          </label>
        )}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button className="vlx-btn" onClick={onCancel}>
            {t("common.cancel")}
          </button>
          <button
            className="vlx-btn vlx-btn-primary"
            style={{ background: "var(--status-error)", borderColor: "var(--status-error)" }}
            onClick={() => onConfirm(removeWt)}
          >
            {t("common.delete")}
          </button>
        </div>
      </div>
    </Backdrop>
  );
}

/**
 * Delete Worktree dialog listing removable worktrees under repoRoot, excluding the primary and bare trees.
 * Select defaultPath when it belongs to the triggering session/group; otherwise require user selection.
 * Force Delete handles uncommitted changes. The dialog owns selection/errors while onConfirm removes it.
 */
export function DeleteWorktree({
  repoRoot,
  defaultPath,
  onConfirm,
  onCancel,
}: {
  repoRoot: string;
  defaultPath: string | null;
  onConfirm: (payload: { path: string; force: boolean }) => Promise<void>;
  onCancel: () => void;
}) {
  const t = useT();
  // Removable worktrees excluding primary and bare entries; null means loading.
  const [list, setList] = useState<WorktreeEntry[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string>("");
  const [pickOpen, setPickOpen] = useState(false);
  const [force, setForce] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load removable worktrees on mount and select the bound path only when present.
  useEffect(() => {
    let alive = true;
    setLoading(true);
    setLoadError(null);
    listWorktrees(repoRoot)
      .then((all) => {
        if (!alive) return;
        const linked = all.filter((w) => !w.isMain && !w.isBare);
        setList(linked);
        if (defaultPath && linked.some((w) => w.path === defaultPath)) {
          setSelected(defaultPath);
        }
      })
      .catch((e) => {
        if (!alive) return;
        setList([]);
        setLoadError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [repoRoot, defaultPath]);

  const canDelete = !busy && selected.trim().length > 0;

  const submit = async () => {
    if (!canDelete) return;
    setBusy(true);
    setError(null);
    try {
      await onConfirm({ path: selected, force });
      // The parent closes on success; failures keep the dialog open through catch.
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  };

  return (
    <Backdrop onClose={onCancel}>
      <div
        style={{
          width: 400,
          background: "var(--bg-panel)",
          border: "1px solid var(--border)",
          borderRadius: 10,
          padding: 18,
          boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)", marginBottom: 10 }}>
          {t("tree.deleteWorktreeTitle")}
        </div>
        <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.6, marginBottom: 16 }}>
          {t("tree.deleteWorktreeBody")}
        </div>

        {/* Worktree picker, reusing the dropdown styling of "choose an existing worktree". */}
        {loading ? (
          <div style={{ fontSize: 12, color: "var(--text-muted)" }}>{t("worktreeSel.loading")}</div>
        ) : list && list.length > 0 ? (
          <div style={{ position: "relative" }}>
            <button
              type="button"
              className="vlx-input"
              onClick={() => setPickOpen((o) => !o)}
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                cursor: "pointer",
                textAlign: "left",
                borderColor: pickOpen ? "var(--accent)" : undefined,
              }}
            >
              <span
                style={{
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                  color: selected ? undefined : "var(--text-muted)",
                }}
              >
                {selected
                  ? worktreeLabel(list.find((w) => w.path === selected))
                  : t("tree.deleteWorktreePlaceholder")}
              </span>
              <Icons.chevD size={14} style={{ color: "var(--text-muted)", flex: "none" }} />
            </button>
            {pickOpen && (
              <>
                <div
                  style={{ position: "fixed", inset: 0, zIndex: 1190 }}
                  onClick={() => setPickOpen(false)}
                />
                <div
                  style={{
                    position: "absolute",
                    top: "calc(100% + 4px)",
                    left: 0,
                    right: 0,
                    zIndex: 1200,
                    maxHeight: 220,
                    overflowY: "auto",
                    padding: 4,
                    background: "var(--bg-panel)",
                    border: "1px solid var(--border)",
                    borderRadius: 8,
                    boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
                  }}
                >
                  {list.map((w) => {
                    const on = w.path === selected;
                    return (
                      <div
                        key={w.path}
                        onClick={() => {
                          setSelected(w.path);
                          setPickOpen(false);
                        }}
                        style={{
                          display: "flex",
                          alignItems: "center",
                          gap: 6,
                          padding: "6px 9px",
                          borderRadius: 5,
                          cursor: "pointer",
                          fontSize: 13,
                          color: on ? "var(--accent)" : "var(--text-primary)",
                          background: on ? "var(--accent-soft)" : "transparent",
                        }}
                        onMouseEnter={(e) => {
                          if (!on) e.currentTarget.style.background = "var(--bg-elevated)";
                        }}
                        onMouseLeave={(e) => {
                          if (!on) e.currentTarget.style.background = "transparent";
                        }}
                      >
                        <span
                          style={{
                            flex: 1,
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                          }}
                        >
                          {worktreeLabel(w)}
                        </span>
                        {on && <Icons.check size={13} style={{ flex: "none" }} />}
                      </div>
                    );
                  })}
                </div>
              </>
            )}
          </div>
        ) : (
          <div style={{ fontSize: 12, color: "var(--text-muted)", lineHeight: 1.5 }}>
            {loadError ? t("worktreeSel.loadFailed") : t("worktreeSel.empty")}
          </div>
        )}

        {/* Force delete: an ordinary delete fails when the working tree has uncommitted changes, so checking this adds --force. */}
        {list && list.length > 0 && (
          <label
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              fontSize: 13,
              color: "var(--text-secondary)",
              lineHeight: 1.5,
              marginTop: 14,
              cursor: "pointer",
            }}
          >
            <input type="checkbox" checked={force} onChange={(e) => setForce(e.target.checked)} />
            {t("tree.deleteWorktreeForce")}
          </label>
        )}

        {/* The delete failure, showing the backend's raw detail. */}
        {error && (
          <div
            style={{
              marginTop: 14,
              padding: "8px 10px",
              borderRadius: 6,
              background: "var(--bg-app)",
              border: "1px solid var(--status-error)",
              fontSize: 11.5,
              color: "var(--text-secondary)",
              lineHeight: 1.5,
              fontFamily: 'Menlo, Monaco, "Courier New", monospace',
              wordBreak: "break-all",
            }}
          >
            {error}
          </div>
        )}

        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 18 }}>
          <button className="vlx-btn" onClick={onCancel}>
            {t("common.cancel")}
          </button>
          <button
            className="vlx-btn vlx-btn-primary"
            style={{ background: "var(--status-error)", borderColor: "var(--status-error)" }}
            onClick={() => void submit()}
            disabled={!canDelete}
          >
            {t("common.delete")}
          </button>
        </div>
      </div>
    </Backdrop>
  );
}

/** Session details including IDs, PID, and working directory, with one-click copying for key fields. */
export function SessionInfo({
  id,
  advanced,
  onClose,
}: {
  id: string;
  /** True when opened with Option/Alt, revealing the backend-composed full launch command. */
  advanced?: boolean;
  onClose: () => void;
}) {
  const t = useT();
  // Suspend native browser views while the dialog is visible so they cannot cover it.
  useSuspendNativeViews();
  // Draft sessions are absent from sessions, so fall back to ephemeralSessions for details.
  const session = useTermStore(
    (s) => s.sessions.find((x) => x.id === id) ?? s.ephemeralSessions[id],
  );
  const project = useTermStore((s) =>
    session ? s.projects.find((p) => p.id === session.projectId) : undefined,
  );
  const runtime = useTermStore((s) => s.runtimes[id]);

  if (!session) return null;

  const isAgent = session.kind !== "terminal";
  const status = effectiveStatus(runtime);
  const cwd = session.cwd || project?.rootPath || null;
  const dim = "—";

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 1100,
        background: "rgba(0,0,0,0.45)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
      onClick={onClose}
    >
      <div
        style={{
          width: 460,
          maxHeight: "80vh",
          overflowY: "auto",
          background: "var(--bg-panel)",
          border: "1px solid var(--border)",
          borderRadius: 10,
          padding: 18,
          boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
        }}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Escape") onClose();
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            marginBottom: 14,
          }}
        >
          <span style={{ color: "var(--text-primary)" }}>{kindIconEl(session.kind, 16)}</span>
          <span
            style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)", flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
          >
            {session.name}
          </span>
          <button className="icon-btn sm" title={t("common.close")} onClick={onClose}>
            <Icons.x size={14} />
          </button>
        </div>

        <InfoRow label={t("info.name")} value={session.name} />
        <InfoRow label={t("info.type")} value={kindLabel(session.kind)} />
        <InfoRow label={t("info.status")} value={statusLabel(status)} />
        {isAgent && (
          <InfoRow
            label="Agent Session ID"
            value={session.agentSessionId || t("info.notYetCaptured")}
            mono={!!session.agentSessionId}
            copy={session.agentSessionId ?? null}
          />
        )}
        <InfoRow
          label={t("info.sessionId")}
          value={session.id}
          mono
          copy={session.id}
        />
        <InfoRow label="PID" value={runtime?.pid ?? dim} mono={!!runtime?.pid} />
        {isAgent && (
          <InfoRow label="agent" value={runtime?.agent || dim} />
        )}
        <InfoRow
          label={t("info.cwd")}
          value={cwd || dim}
          mono={!!cwd}
          copy={cwd}
        />
        {session.shell && <InfoRow label="Shell" value={session.shell} mono />}
        {session.initCmd && (
          <InfoRow label={t("info.initCmd")} value={session.initCmd} mono copy={session.initCmd} />
        )}
        {session.agentArgs && (
          <InfoRow label={t("info.agentArgs")} value={session.agentArgs} mono copy={session.agentArgs} />
        )}
        {session.permissionMode === "skip" && (
          <InfoRow label={t("info.permission")} value={t("info.permissionSkip")} />
        )}
        {/* Advanced view, opened by holding Option: the full launch command the backend actually wrote to
            the PTY for this spawn. It only has a value after an agent session spawns successfully; the
            whole row is hidden for plain terminals, sessions that never started, and mirror clients. */}
        {advanced && runtime?.launchCmd && (
          <InfoRow
            label={t("info.launchCmd")}
            value={runtime.launchCmd}
            mono
            copy={runtime.launchCmd}
          />
        )}
        {session.parentSessionId && (
          <InfoRow
            label={t("info.parentSessionId")}
            value={session.parentSessionId}
            mono
            copy={session.parentSessionId}
          />
        )}
        {session.worktreePath && (
          <InfoRow
            label="Worktree"
            value={session.worktreePath}
            mono
            copy={session.worktreePath}
          />
        )}
        {runtime?.title && <InfoRow label={t("info.termTitle")} value={runtime.title} />}
        <InfoRow
          label={t("info.createdAt")}
          // createdAt is stored as Unix seconds; Date expects milliseconds.
          value={new Date(session.createdAt * 1000).toLocaleString(dateLocale())}
        />

        <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 16 }}>
          <button className="vlx-btn vlx-btn-primary" onClick={onClose}>
            {t("common.close")}
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * Group Information dialog using the same centered InfoRow shell as Session Information. Available only
 * for worktree groups, it shows name, branch, full worktree path, and recent commits. useGitBranchInfo
 * typically hits its path cache, while gitRecentCommits queries asynchronously on a blocking worker so the
 * dialog appears before the commit list finishes loading.
 */
export function GroupInfo({ id, onClose }: { id: string; onClose: () => void }) {
  const t = useT();
  // Suspend native browser views while the dialog is visible so they cannot cover it.
  useSuspendNativeViews();
  const group = useTermStore((s) => s.groups.find((g) => g.id === id));
  const worktreePath = group?.worktreePath ?? null;
  // Branch from the path-cached Git probe shared with sidebar badges.
  const branchInfo = useGitBranchInfo(worktreePath);
  // Recent commits queried asynchronously: null loading, empty none, nonempty loaded.
  const [commits, setCommits] = useState<CommitInfo[] | null>(null);
  const [commitsErr, setCommitsErr] = useState(false);

  useEffect(() => {
    // An unexpected missing worktree falls back to no commits rather than perpetual loading.
    if (!worktreePath) {
      setCommits([]);
      setCommitsErr(false);
      return;
    }
    let alive = true;
    setCommits(null);
    setCommitsErr(false);
    gitRecentCommits(worktreePath)
      .then((list) => {
        if (alive) setCommits(list);
      })
      .catch(() => {
        if (alive) setCommitsErr(true);
      });
    return () => {
      alive = false;
    };
  }, [worktreePath]);

  if (!group) return null;
  const dim = "—";

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 1100,
        background: "rgba(0,0,0,0.45)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
      onClick={onClose}
    >
      <div
        style={{
          width: 460,
          maxHeight: "80vh",
          overflowY: "auto",
          background: "var(--bg-panel)",
          border: "1px solid var(--border)",
          borderRadius: 10,
          padding: 18,
          boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
        }}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Escape") onClose();
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 8,
            marginBottom: 14,
          }}
        >
          <span
            style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)", flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
          >
            {group.name}
          </span>
          <button className="icon-btn sm" title={t("common.close")} onClick={onClose}>
            <Icons.x size={14} />
          </button>
        </div>

        <InfoRow label={t("info.name")} value={group.name} />
        <InfoRow
          label={t("info.branch")}
          value={branchInfo.branch || dim}
          mono={!!branchInfo.branch}
          copy={branchInfo.branch}
        />
        <InfoRow
          label={t("info.path")}
          value={worktreePath || dim}
          mono={!!worktreePath}
          copy={worktreePath}
        />

        <div style={{ marginTop: 14 }}>
          <div style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 8 }}>
            {t("info.recentCommits")}
          </div>
          {commits === null && !commitsErr && (
            <div style={{ fontSize: 12.5, color: "var(--text-muted)" }}>{t("common.loading")}</div>
          )}
          {commitsErr && (
            <div style={{ fontSize: 12.5, color: "var(--text-muted)" }}>{dim}</div>
          )}
          {commits !== null && commits.length === 0 && (
            <div style={{ fontSize: 12.5, color: "var(--text-muted)" }}>{t("info.noCommits")}</div>
          )}
          {commits !== null && commits.length > 0 && (
            <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
              {commits.map((c) => (
                <div key={c.hash} style={{ display: "flex", gap: 8, alignItems: "baseline" }}>
                  <span
                    style={{
                      flex: "0 0 auto",
                      fontFamily: 'Menlo, Monaco, "Courier New", monospace',
                      fontSize: 11.5,
                      color: "var(--text-secondary)",
                    }}
                  >
                    {c.hash}
                  </span>
                  <span style={{ flex: 1, minWidth: 0 }}>
                    <span style={{ fontSize: 12.5, color: "var(--text-primary)", wordBreak: "break-word" }}>
                      {c.subject}
                    </span>
                    <span style={{ display: "block", fontSize: 11, color: "var(--text-muted)", marginTop: 2 }}>
                      {c.author}
                      {c.author && c.relative ? " · " : ""}
                      {c.relative}
                    </span>
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>

        <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 16 }}>
          <button className="vlx-btn vlx-btn-primary" onClick={onClose}>
            {t("common.close")}
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * New Group dialog with a name and worktree choice: none, create, or existing. Worktree groups display a
 * sidebar tag and pass the worktree to new sessions. In create mode the parent creates an independent Git
 * worktree before the group; failures remain visible without creating or closing.
 */
export function NewGroup({
  projectId,
  parentGroupId,
  onConfirm,
  onCancel,
}: {
  projectId: string;
  parentGroupId: string | null;
  onConfirm: (payload: { name: string; worktree: WorktreeChoice }) => Promise<void>;
  onCancel: () => void;
}) {
  const t = useT();
  const [name, setName] = useState("");

  // Worktree mode: none by default, create new, or attach existing.
  const [wtMode, setWtMode] = useState<"none" | "new" | "existing">("none");
  // New worktree directory/branch name follows the group name until edited independently.
  const [worktreeName, setWorktreeName] = useState("");
  const [worktreeNameEdited, setWorktreeNameEdited] = useState(false);
  // Existing worktrees loaded lazily when switching to existing mode.
  const [existing, setExisting] = useState<WorktreeEntry[] | null>(null);
  const [wtLoading, setWtLoading] = useState(false);
  const [wtLoadError, setWtLoadError] = useState<string | null>(null);
  const [selectedWt, setSelectedWt] = useState<string>("");
  const [wtPickOpen, setWtPickOpen] = useState(false);

  // Resolve repository root once from a parent group's worktree or the project root.
  const [repoRoot] = useState<string | null>(() => {
    const st = useTermStore.getState();
    const parentGroup = parentGroupId
      ? st.groups.find((g) => g.id === parentGroupId)
      : null;
    const project = st.projects.find((p) => p.id === projectId);
    return parentGroup?.worktreePath || project?.rootPath || null;
  });

  // On first existing-mode entry, load worktrees excluding primary and bare entries.
  useEffect(() => {
    if (wtMode !== "existing" || existing !== null || !repoRoot) return;
    let alive = true;
    setWtLoading(true);
    setWtLoadError(null);
    listWorktrees(repoRoot)
      .then((list) => {
        if (!alive) return;
        const linked = list.filter((w) => !w.isMain && !w.isBare);
        setExisting(linked);
        if (linked.length > 0) setSelectedWt((cur) => cur || linked[0].path);
      })
      .catch((e) => {
        if (!alive) return;
        setExisting([]);
        setWtLoadError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        if (alive) setWtLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [wtMode, existing, repoRoot]);

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Confirmation requires a group name and, in existing mode, a selected worktree.
  const canSubmit =
    !busy &&
    name.trim().length > 0 &&
    (wtMode !== "existing" || selectedWt.trim().length > 0);

  const submit = async () => {
    if (!canSubmit) return;
    const finalName = name.trim();
    let worktree: WorktreeChoice;
    if (wtMode === "new") {
      // An empty worktree name falls back to the group name before backend slugification.
      worktree = { mode: "new", name: worktreeName.trim() || finalName };
    } else if (wtMode === "existing") {
      worktree = { mode: "existing", path: selectedWt };
    } else {
      worktree = { mode: "none" };
    }
    setBusy(true);
    setError(null);
    try {
      await onConfirm({ name: finalName, worktree });
      // The parent closes on success; catch leaves the dialog open on failure.
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  };

  return (
    <Backdrop onClose={onCancel}>
      <div
        style={{
          width: 400,
          background: "var(--bg-panel)",
          border: "1px solid var(--border)",
          borderRadius: 10,
          padding: 18,
          boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
        }}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) void submit();
          if (e.key === "Escape") onCancel();
        }}
      >
        <div
          style={{
            fontSize: 14,
            fontWeight: 600,
            color: "var(--text-primary)",
            marginBottom: 14,
          }}
        >
          {t("tree.newGroup")}
        </div>

        {/* Group name. */}
        <label style={{ display: "block", marginBottom: 12 }}>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>
            {t("tree.groupName")}
            <span style={{ color: "var(--status-error)", marginLeft: 3 }}>*</span>
          </div>
          <input
            className="vlx-input"
            autoFocus
            autoCapitalize="none"
            value={name}
            onChange={(e) => {
              const v = e.target.value;
              setName(v);
              // Keep the worktree name synchronized until the user edits it independently.
              if (!worktreeNameEdited) setWorktreeName(v);
            }}
          />
        </label>

        {/* Worktree choice: none (the default), create a new one, or attach to an existing one. */}
        <div style={{ marginBottom: 12 }}>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>
            {t("worktreeSel.label")}
          </div>
          <div style={{ display: "flex", gap: 6 }}>
            {(["none", "new", "existing"] as const).map((m) => {
              const on = wtMode === m;
              const label =
                m === "none"
                  ? t("worktreeSel.modeNone")
                  : m === "new"
                    ? t("worktreeSel.modeNew")
                    : t("worktreeSel.modeExisting");
              return (
                <button
                  key={m}
                  type="button"
                  className="vlx-input"
                  onClick={() => setWtMode(m)}
                  style={{
                    flex: 1,
                    cursor: "pointer",
                    textAlign: "center",
                    color: on ? "var(--accent)" : "var(--text-primary)",
                    borderColor: on ? "var(--accent)" : undefined,
                    background: on ? "var(--accent-soft)" : undefined,
                  }}
                >
                  {label}
                </button>
              );
            })}
          </div>

          {/* Create mode: the worktree name, used for both directory and branch. It follows the group name by default and can be changed separately. */}
          {wtMode === "new" && (
            <label style={{ display: "block", marginTop: 10 }}>
              <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>
                {t("worktree.worktreeNameLabel")}
              </div>
              <input
                className="vlx-input"
                autoCapitalize="none"
                placeholder={name.trim() || undefined}
                value={worktreeName}
                onChange={(e) => {
                  setWorktreeName(e.target.value);
                  setWorktreeNameEdited(true);
                }}
              />
              <div
                style={{
                  fontSize: 11,
                  color: "var(--text-muted)",
                  lineHeight: 1.5,
                  marginTop: 4,
                }}
              >
                {t("worktree.worktreeNameHint")}
              </div>
            </label>
          )}

          {/* Attach mode: list the worktrees that already exist in the repository, excluding the main working tree and bare repositories. */}
          {wtMode === "existing" && (
            <div style={{ marginTop: 10 }}>
              {wtLoading ? (
                <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
                  {t("worktreeSel.loading")}
                </div>
              ) : existing && existing.length > 0 ? (
                <div style={{ position: "relative" }}>
                  <button
                    type="button"
                    className="vlx-input"
                    onClick={() => setWtPickOpen((o) => !o)}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      cursor: "pointer",
                      textAlign: "left",
                      borderColor: wtPickOpen ? "var(--accent)" : undefined,
                    }}
                  >
                    <span
                      style={{
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                    >
                      {worktreeLabel(existing.find((w) => w.path === selectedWt))}
                    </span>
                    <Icons.chevD
                      size={14}
                      style={{ color: "var(--text-muted)", flex: "none" }}
                    />
                  </button>
                  {wtPickOpen && (
                    <>
                      <div
                        style={{ position: "fixed", inset: 0, zIndex: 1190 }}
                        onClick={() => setWtPickOpen(false)}
                      />
                      <div
                        style={{
                          position: "absolute",
                          top: "calc(100% + 4px)",
                          left: 0,
                          right: 0,
                          zIndex: 1200,
                          maxHeight: 220,
                          overflowY: "auto",
                          padding: 4,
                          background: "var(--bg-panel)",
                          border: "1px solid var(--border)",
                          borderRadius: 8,
                          boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
                        }}
                      >
                        {existing.map((w) => {
                          const on = w.path === selectedWt;
                          return (
                            <div
                              key={w.path}
                              onClick={() => {
                                setSelectedWt(w.path);
                                setWtPickOpen(false);
                              }}
                              style={{
                                display: "flex",
                                alignItems: "center",
                                gap: 6,
                                padding: "6px 9px",
                                borderRadius: 5,
                                cursor: "pointer",
                                fontSize: 13,
                                color: on ? "var(--accent)" : "var(--text-primary)",
                                background: on ? "var(--accent-soft)" : "transparent",
                              }}
                              onMouseEnter={(e) => {
                                if (!on)
                                  e.currentTarget.style.background = "var(--bg-elevated)";
                              }}
                              onMouseLeave={(e) => {
                                if (!on) e.currentTarget.style.background = "transparent";
                              }}
                            >
                              <span
                                style={{
                                  flex: 1,
                                  overflow: "hidden",
                                  textOverflow: "ellipsis",
                                  whiteSpace: "nowrap",
                                }}
                              >
                                {worktreeLabel(w)}
                              </span>
                              {on && <Icons.check size={13} style={{ flex: "none" }} />}
                            </div>
                          );
                        })}
                      </div>
                    </>
                  )}
                </div>
              ) : (
                <div style={{ fontSize: 12, color: "var(--text-muted)", lineHeight: 1.5 }}>
                  {wtLoadError ? t("worktreeSel.loadFailed") : t("worktreeSel.empty")}
                </div>
              )}
            </div>
          )}

          {/* When a worktree is attached, note that new sessions in this group inherit it by default. */}
          {wtMode !== "none" && (
            <div
              style={{
                fontSize: 11,
                color: "var(--text-muted)",
                lineHeight: 1.5,
                marginTop: 8,
              }}
            >
              {t("group.worktreeHint")}
            </div>
          )}
        </div>

        {/* Worktree creation failure: a localised title plus the backend's raw detail. */}
        {error && (
          <div
            style={{
              marginTop: 14,
              padding: "8px 10px",
              borderRadius: 6,
              background: "var(--bg-app)",
              border: "1px solid var(--status-error)",
            }}
          >
            <div style={{ fontSize: 12, fontWeight: 600, color: "var(--status-error)" }}>
              {t("worktree.createFailed")}
            </div>
            <div
              style={{
                fontSize: 11.5,
                color: "var(--text-secondary)",
                lineHeight: 1.5,
                marginTop: 3,
                fontFamily: 'Menlo, Monaco, "Courier New", monospace',
                wordBreak: "break-all",
              }}
            >
              {error}
            </div>
          </div>
        )}

        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 18 }}>
          <button className="vlx-btn" onClick={onCancel}>
            {t("common.cancel")}
          </button>
          <button
            className="vlx-btn vlx-btn-primary"
            onClick={() => void submit()}
            disabled={!canSubmit}
          >
            {t("common.create")}
          </button>
        </div>
      </div>
    </Backdrop>
  );
}

/**
 * Create with Custom Arguments dialog for agent type, optional session name/arguments, and none/new/existing
 * worktree. Ordinary creation remains immediate. Arguments persist in session configuration, apply to every
 * launch or resume, remain editable, and are split into words before being appended unchanged.
 */
export function NewAgentSession({
  projectId,
  parentSessionId,
  initialWtMode = "none",
  onConfirm,
  onCancel,
}: {
  projectId: string;
  parentSessionId: string | null;
  initialWtMode?: "none" | "new" | "existing";
  onConfirm: (opts: {
    kind: SessionKind;
    name: string;
    agentArgs: string;
    permissionMode: string | null;
    worktree: WorktreeChoice;
  }) => Promise<void>;
  onCancel: () => void;
}) {
  const t = useT();
  const agentDefaults = useTermStore((s) => s.agentDefaults);
  const [kind, setKind] = useState<SessionKind>("claude");
  const [typeOpen, setTypeOpen] = useState(false);
  const [name, setName] = useState("");
  // Prefill arguments and permissions from this type's global agent template; users may override or clear.
  const [agentArgs, setAgentArgs] = useState(() => agentDefaults["claude"]?.args ?? "");
  const [skipPerm, setSkipPerm] = useState(
    () => agentDefaults["claude"]?.permissionMode === "skip",
  );

  // ── Worktree selection: none at project root, create new, or attach existing. initialWtMode distinguishes
  // custom-argument creation's default none from New Worktree Session's new mode. ──
  const [wtMode, setWtMode] = useState<"none" | "new" | "existing">(initialWtMode);
  // New worktree directory/branch name follows the session name until independently edited.
  const [worktreeName, setWorktreeName] = useState("");
  const [worktreeNameEdited, setWorktreeNameEdited] = useState(false);
  // Existing worktrees loaded lazily on first entry into existing mode.
  const [existing, setExisting] = useState<WorktreeEntry[] | null>(null);
  const [wtLoading, setWtLoading] = useState(false);
  const [wtLoadError, setWtLoadError] = useState<string | null>(null);
  const [selectedWt, setSelectedWt] = useState<string>("");
  const [wtPickOpen, setWtPickOpen] = useState(false);

  // Resolve repository root once from the parent session directory first, otherwise project root.
  const [repoRoot] = useState<string | null>(() => {
    const st = useTermStore.getState();
    const parent = parentSessionId
      ? st.sessions.find((s) => s.id === parentSessionId)
      : null;
    const project = st.projects.find((p) => p.id === projectId);
    return parent?.cwd || project?.rootPath || null;
  });

  // Default session name uses the highest same-kind project number plus one.
  const autoName = (k: SessionKind): string => {
    const all = useTermStore.getState().sessions;
    const label = kindLabel(k);
    const re = new RegExp(`^${label} (\\d+)$`);
    const maxN = all
      .filter((s) => s.projectId === projectId && s.kind === k)
      .reduce((m, s) => {
        const mt = re.exec(s.name);
        return mt ? Math.max(m, Number(mt[1])) : m;
      }, 0);
    return `${label} ${maxN + 1}`;
  };

  // Load existing worktrees once, excluding primary and bare entries.
  useEffect(() => {
    if (wtMode !== "existing" || existing !== null || !repoRoot) return;
    let alive = true;
    setWtLoading(true);
    setWtLoadError(null);
    listWorktrees(repoRoot)
      .then((list) => {
        if (!alive) return;
        const linked = list.filter((w) => !w.isMain && !w.isBare);
        setExisting(linked);
        if (linked.length > 0) setSelectedWt((cur) => cur || linked[0].path);
      })
      .catch((e) => {
        if (!alive) return;
        setExisting([]);
        setWtLoadError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        if (alive) setWtLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [wtMode, existing, repoRoot]);

  // When type changes, replace arguments only if still equal to the old default or empty; preserve user edits.
  // The binary permission toggle can safely follow the new type's default.
  const changeKind = (k: SessionKind) => {
    setTypeOpen(false);
    setAgentArgs((cur) =>
      cur.trim() === "" || cur === (agentDefaults[kind]?.args ?? "")
        ? agentDefaults[k]?.args ?? ""
        : cur,
    );
    setSkipPerm(agentDefaults[k]?.permissionMode === "skip");
    setKind(k);
  };
  // Permission-toggle support follows the selected agent type; OpenCode has no corresponding flag.
  const permSupported = supportsPermissionToggle(kind);

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // New mode always confirms because an empty worktree name falls back; existing requires a selection.
  const canSubmit =
    !busy && (wtMode !== "existing" || selectedWt.trim().length > 0);

  const submit = async () => {
    if (busy || !canSubmit) return;
    const finalName = name.trim() || autoName(kind);
    let worktree: WorktreeChoice;
    if (wtMode === "new") {
      worktree = { mode: "new", name: worktreeName.trim() || finalName };
    } else if (wtMode === "existing") {
      worktree = { mode: "existing", path: selectedWt };
    } else {
      worktree = { mode: "none" };
    }
    setBusy(true);
    setError(null);
    try {
      // Restore em/en dashes to `--` after macOS smart punctuation alters command-line flags.
      await onConfirm({
        kind,
        name: finalName,
        agentArgs: normalizeArgDashes(agentArgs),
        permissionMode: permSupported && skipPerm ? "skip" : null,
        worktree,
      });
      // The parent closes on success; catch keeps the dialog open on failure.
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  };

  return (
    <Backdrop onClose={onCancel}>
      <div
        style={{
          width: 400,
          background: "var(--bg-panel)",
          border: "1px solid var(--border)",
          borderRadius: 10,
          padding: 18,
          boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
        }}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) void submit();
          if (e.key === "Escape") onCancel();
        }}
      >
        <div
          style={{
            fontSize: 14,
            fontWeight: 600,
            color: "var(--text-primary)",
            marginBottom: 4,
          }}
        >
          {t("tree.newAgentSessionGroup")}
        </div>
        <div
          style={{
            fontSize: 12,
            color: "var(--text-muted)",
            lineHeight: 1.6,
            marginBottom: 14,
          }}
        >
          {t("newAgent.desc")}
        </div>

        <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>
          {t("resume.agentType")}
        </div>
        {/* A custom dropdown rather than a native <select>: WKWebView forces the latter into a light,
            raised box that clashes with the theme. This matches the language dropdown in Appearance
            settings. Clicking the backdrop closes only the dropdown, never the surrounding dialog: the
            backdrop is a DOM child of the panel, whose stopPropagation absorbs the bubbling event. */}
        <div style={{ position: "relative", marginBottom: 12 }}>
          <button
            type="button"
            className="vlx-input"
            onClick={() => setTypeOpen((o) => !o)}
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              cursor: "pointer",
              textAlign: "left",
              borderColor: typeOpen ? "var(--accent)" : undefined,
            }}
          >
            <span>{kindLabel(kind)}</span>
            <Icons.chevD size={14} style={{ color: "var(--text-muted)", flex: "none" }} />
          </button>
          {typeOpen && (
            <>
              <div
                style={{ position: "fixed", inset: 0, zIndex: 1190 }}
                onClick={() => setTypeOpen(false)}
              />
              <div
                style={{
                  position: "absolute",
                  top: "calc(100% + 4px)",
                  left: 0,
                  right: 0,
                  zIndex: 1200,
                  padding: 4,
                  background: "var(--bg-panel)",
                  border: "1px solid var(--border)",
                  borderRadius: 8,
                  boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
                }}
              >
                {AGENT_ARGS_KINDS.map((k) => {
                  const on = k === kind;
                  return (
                    <div
                      key={k}
                      onClick={() => changeKind(k)}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: 6,
                        padding: "6px 9px",
                        borderRadius: 5,
                        cursor: "pointer",
                        fontSize: 13,
                        color: on ? "var(--accent)" : "var(--text-primary)",
                        background: on ? "var(--accent-soft)" : "transparent",
                      }}
                      onMouseEnter={(e) => {
                        if (!on)
                          e.currentTarget.style.background =
                            "var(--bg-elevated)";
                      }}
                      onMouseLeave={(e) => {
                        if (!on) e.currentTarget.style.background = "transparent";
                      }}
                    >
                      <span style={{ flex: 1 }}>{kindLabel(k)}</span>
                      {on && <Icons.check size={13} style={{ flex: "none" }} />}
                    </div>
                  );
                })}
              </div>
            </>
          )}
        </div>

        <label style={{ display: "block", marginBottom: 12 }}>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>
            {t("tree.sessionNameAuto")}
          </div>
          <input
            className="vlx-input"
            autoCapitalize="none"
            placeholder={`${kindLabel(kind)} N`}
            value={name}
            onChange={(e) => {
              const v = e.target.value;
              setName(v);
              // In new-worktree mode, follow the session name until independently edited.
              if (!worktreeNameEdited) setWorktreeName(v);
            }}
          />
        </label>

        {/* Worktree choice: none (the default, running in the project root), create a new one, or attach to an existing one. */}
        <div style={{ marginBottom: 12 }}>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>
            {t("worktreeSel.label")}
          </div>
          <div style={{ display: "flex", gap: 6 }}>
            {(["none", "new", "existing"] as const).map((m) => {
              const on = wtMode === m;
              const label =
                m === "none"
                  ? t("worktreeSel.modeNone")
                  : m === "new"
                    ? t("worktreeSel.modeNew")
                    : t("worktreeSel.modeExisting");
              return (
                <button
                  key={m}
                  type="button"
                  className="vlx-input"
                  onClick={() => setWtMode(m)}
                  style={{
                    flex: 1,
                    cursor: "pointer",
                    textAlign: "center",
                    color: on ? "var(--accent)" : "var(--text-primary)",
                    borderColor: on ? "var(--accent)" : undefined,
                    background: on ? "var(--accent-soft)" : undefined,
                  }}
                >
                  {label}
                </button>
              );
            })}
          </div>

          {/* Create mode: the worktree name, used for both directory and branch. It follows the session name by default and can be changed separately. */}
          {wtMode === "new" && (
            <label style={{ display: "block", marginTop: 10 }}>
              <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>
                {t("worktree.worktreeNameLabel")}
              </div>
              <input
                className="vlx-input"
                autoCapitalize="none"
                placeholder={autoName(kind)}
                value={worktreeName}
                onChange={(e) => {
                  setWorktreeName(e.target.value);
                  setWorktreeNameEdited(true);
                }}
              />
              <div
                style={{
                  fontSize: 11,
                  color: "var(--text-muted)",
                  lineHeight: 1.5,
                  marginTop: 4,
                }}
              >
                {t("worktree.worktreeNameHint")}
              </div>
            </label>
          )}

          {/* Attach mode: list the worktrees that already exist in the repository, excluding the main working tree and bare repositories. */}
          {wtMode === "existing" && (
            <div style={{ marginTop: 10 }}>
              {wtLoading ? (
                <div style={{ fontSize: 12, color: "var(--text-muted)" }}>
                  {t("worktreeSel.loading")}
                </div>
              ) : existing && existing.length > 0 ? (
                <div style={{ position: "relative" }}>
                  <button
                    type="button"
                    className="vlx-input"
                    onClick={() => setWtPickOpen((o) => !o)}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "space-between",
                      cursor: "pointer",
                      textAlign: "left",
                      borderColor: wtPickOpen ? "var(--accent)" : undefined,
                    }}
                  >
                    <span
                      style={{
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                    >
                      {worktreeLabel(existing.find((w) => w.path === selectedWt))}
                    </span>
                    <Icons.chevD
                      size={14}
                      style={{ color: "var(--text-muted)", flex: "none" }}
                    />
                  </button>
                  {wtPickOpen && (
                    <>
                      <div
                        style={{ position: "fixed", inset: 0, zIndex: 1190 }}
                        onClick={() => setWtPickOpen(false)}
                      />
                      <div
                        style={{
                          position: "absolute",
                          top: "calc(100% + 4px)",
                          left: 0,
                          right: 0,
                          zIndex: 1200,
                          maxHeight: 220,
                          overflowY: "auto",
                          padding: 4,
                          background: "var(--bg-panel)",
                          border: "1px solid var(--border)",
                          borderRadius: 8,
                          boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
                        }}
                      >
                        {existing.map((w) => {
                          const on = w.path === selectedWt;
                          return (
                            <div
                              key={w.path}
                              onClick={() => {
                                setSelectedWt(w.path);
                                setWtPickOpen(false);
                              }}
                              style={{
                                display: "flex",
                                alignItems: "center",
                                gap: 6,
                                padding: "6px 9px",
                                borderRadius: 5,
                                cursor: "pointer",
                                fontSize: 13,
                                color: on ? "var(--accent)" : "var(--text-primary)",
                                background: on ? "var(--accent-soft)" : "transparent",
                              }}
                              onMouseEnter={(e) => {
                                if (!on)
                                  e.currentTarget.style.background = "var(--bg-elevated)";
                              }}
                              onMouseLeave={(e) => {
                                if (!on) e.currentTarget.style.background = "transparent";
                              }}
                            >
                              <span
                                style={{
                                  flex: 1,
                                  overflow: "hidden",
                                  textOverflow: "ellipsis",
                                  whiteSpace: "nowrap",
                                }}
                              >
                                {worktreeLabel(w)}
                              </span>
                              {on && <Icons.check size={13} style={{ flex: "none" }} />}
                            </div>
                          );
                        })}
                      </div>
                    </>
                  )}
                </div>
              ) : (
                <div
                  style={{ fontSize: 12, color: "var(--text-muted)", lineHeight: 1.5 }}
                >
                  {wtLoadError ? t("worktreeSel.loadFailed") : t("worktreeSel.empty")}
                </div>
              )}
            </div>
          )}
        </div>

        <label style={{ display: "block" }}>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>
            {t("tree.agentArgsLabel")}
          </div>
          <input
            className="vlx-input"
            autoFocus
            autoCapitalize="none"
            placeholder="--model opus"
            value={agentArgs}
            onChange={(e) => setAgentArgs(e.target.value)}
          />
        </label>

        {/* Two-position switch for the permission mode. opencode has no matching flag, so it is disabled with a short explanation. */}
        <div style={{ marginTop: 12 }}>
          <label
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              fontSize: 13,
              color: permSupported ? "var(--text-primary)" : "var(--text-muted)",
              cursor: permSupported ? "pointer" : "default",
            }}
          >
            <input
              type="checkbox"
              disabled={!permSupported}
              checked={permSupported && skipPerm}
              onChange={(e) => setSkipPerm(e.target.checked)}
            />
            {t("tree.permissionSkipLabel")}
          </label>
          <div
            style={{
              fontSize: 11,
              color: "var(--text-muted)",
              lineHeight: 1.5,
              marginTop: 4,
              marginLeft: 24,
            }}
          >
            {permSupported
              ? t("tree.permissionSkipHint")
              : kind === "pi"
                ? t("tree.permissionUnsupportedPi")
                : t("tree.permissionUnsupported")}
          </div>
        </div>

        {/* Failure to create or attach a worktree: a localised title plus the backend's raw detail. */}
        {error && (
          <div
            style={{
              marginTop: 14,
              padding: "8px 10px",
              borderRadius: 6,
              background: "var(--bg-app)",
              border: "1px solid var(--status-error)",
            }}
          >
            <div style={{ fontSize: 12, fontWeight: 600, color: "var(--status-error)" }}>
              {t("worktree.createFailed")}
            </div>
            <div
              style={{
                fontSize: 11.5,
                color: "var(--text-secondary)",
                lineHeight: 1.5,
                marginTop: 3,
                fontFamily: 'Menlo, Monaco, "Courier New", monospace',
                wordBreak: "break-all",
              }}
            >
              {error}
            </div>
          </div>
        )}

        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 18 }}>
          <button className="vlx-btn" onClick={onCancel}>
            {t("common.cancel")}
          </button>
          <button
            className="vlx-btn vlx-btn-primary"
            onClick={() => void submit()}
            disabled={!canSubmit}
          >
            {t("common.create")}
          </button>
        </div>
      </div>
    </Backdrop>
  );
}

/**
 * Restore Session dialog selecting an agent type and native session ID to create a preset resume anchor.
 * Existing automatic logic resumes it as-is; backend validation falls back to a new launch if it no longer
 * exists. Optional arguments and skip-permission mode use global defaults, persist, and remain editable.
 */
export function ResumeSession({
  onConfirm,
  onCancel,
}: {
  onConfirm: (opts: {
    kind: SessionKind;
    sessionId: string;
    name: string;
    agentArgs: string;
    permissionMode: string | null;
  }) => void;
  onCancel: () => void;
}) {
  const t = useT();
  const agentDefaults = useTermStore((s) => s.agentDefaults);
  const [kind, setKind] = useState<SessionKind>("claude");
  const [typeOpen, setTypeOpen] = useState(false);
  const [sessionId, setSessionId] = useState("");
  const [name, setName] = useState("");
  // Prefill arguments and permissions from the type's global agent template; users may override or clear.
  const [agentArgs, setAgentArgs] = useState(() => agentDefaults["claude"]?.args ?? "");
  const [skipPerm, setSkipPerm] = useState(
    () => agentDefaults["claude"]?.permissionMode === "skip",
  );
  // On type change, replace arguments only if still at the old default or empty; preserve user edits.
  // The binary permission toggle safely follows the new type's default.
  const changeKind = (k: SessionKind) => {
    setTypeOpen(false);
    setAgentArgs((cur) =>
      cur.trim() === "" || cur === (agentDefaults[kind]?.args ?? "")
        ? agentDefaults[k]?.args ?? ""
        : cur,
    );
    setSkipPerm(agentDefaults[k]?.permissionMode === "skip");
    setKind(k);
  };
  // Permission-toggle support follows the selected agent type; OpenCode has no corresponding flag.
  const permSupported = supportsPermissionToggle(kind);
  const canSubmit = sessionId.trim().length > 0;
  const submit = () => {
    if (!canSubmit) return;
    // Restore em/en dashes to `--` after macOS smart punctuation alters command-line flags.
    onConfirm({
      kind,
      sessionId,
      name,
      agentArgs: normalizeArgDashes(agentArgs),
      permissionMode: permSupported && skipPerm ? "skip" : null,
    });
  };

  return (
    <Backdrop onClose={onCancel}>
      <div
        style={{
          width: 400,
          background: "var(--bg-panel)",
          border: "1px solid var(--border)",
          borderRadius: 10,
          padding: 18,
          boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
        }}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) submit();
          if (e.key === "Escape") onCancel();
        }}
      >
        <div
          style={{
            fontSize: 14,
            fontWeight: 600,
            color: "var(--text-primary)",
            marginBottom: 4,
          }}
        >
          {t("resume.title")}
        </div>
        <div
          style={{
            fontSize: 12,
            color: "var(--text-muted)",
            lineHeight: 1.6,
            marginBottom: 14,
          }}
        >
          {t("resume.desc")}
        </div>

        <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>
          {t("resume.agentType")}
        </div>
        {/* A custom dropdown rather than a native <select>: WKWebView forces the latter into a light,
            raised box that clashes with the theme. This matches the dropdowns in "new session with
            custom arguments" and Appearance settings. Clicking the backdrop closes only the dropdown,
            never the surrounding dialog: the backdrop is a DOM child of the panel, whose
            stopPropagation absorbs the bubbling event. */}
        <div style={{ position: "relative", marginBottom: 12 }}>
          <button
            type="button"
            className="vlx-input"
            onClick={() => setTypeOpen((o) => !o)}
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              cursor: "pointer",
              textAlign: "left",
              borderColor: typeOpen ? "var(--accent)" : undefined,
            }}
          >
            <span>{kindLabel(kind)}</span>
            <Icons.chevD size={14} style={{ color: "var(--text-muted)", flex: "none" }} />
          </button>
          {typeOpen && (
            <>
              <div
                style={{ position: "fixed", inset: 0, zIndex: 1190 }}
                onClick={() => setTypeOpen(false)}
              />
              <div
                style={{
                  position: "absolute",
                  top: "calc(100% + 4px)",
                  left: 0,
                  right: 0,
                  zIndex: 1200,
                  padding: 4,
                  background: "var(--bg-panel)",
                  border: "1px solid var(--border)",
                  borderRadius: 8,
                  boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
                }}
              >
                {RESUMABLE_KINDS.map((opt) => {
                  const on = opt.kind === kind;
                  return (
                    <div
                      key={opt.kind}
                      onClick={() => changeKind(opt.kind)}
                      style={{
                        display: "flex",
                        alignItems: "center",
                        gap: 6,
                        padding: "6px 9px",
                        borderRadius: 5,
                        cursor: "pointer",
                        fontSize: 13,
                        color: on ? "var(--accent)" : "var(--text-primary)",
                        background: on ? "var(--accent-soft)" : "transparent",
                      }}
                      onMouseEnter={(e) => {
                        if (!on)
                          e.currentTarget.style.background =
                            "var(--bg-elevated)";
                      }}
                      onMouseLeave={(e) => {
                        if (!on) e.currentTarget.style.background = "transparent";
                      }}
                    >
                      <span style={{ flex: 1 }}>{opt.label}</span>
                      {on && <Icons.check size={13} style={{ flex: "none" }} />}
                    </div>
                  );
                })}
              </div>
            </>
          )}
        </div>

        <label style={{ display: "block", marginBottom: 12 }}>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>
            Session ID<span style={{ color: "var(--status-error)" }}> *</span>
          </div>
          <input
            className="vlx-input"
            autoFocus
            autoCapitalize="none"
            placeholder={kind === "opencode" ? "ses_..." : t("resume.sessionIdPlaceholder")}
            value={sessionId}
            onChange={(e) => setSessionId(e.target.value)}
          />
        </label>

        <label style={{ display: "block", marginBottom: 12 }}>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>
            {t("tree.sessionNameAuto")}
          </div>
          <input
            className="vlx-input"
            autoCapitalize="none"
            placeholder={`${kindLabel(kind)} N`}
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        </label>

        <label style={{ display: "block" }}>
          <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>
            {t("tree.agentArgsLabel")}
          </div>
          <input
            className="vlx-input"
            autoCapitalize="none"
            placeholder="--model opus"
            value={agentArgs}
            onChange={(e) => setAgentArgs(e.target.value)}
          />
        </label>

        {/* Two-position switch for the permission mode. opencode has no matching flag, so it is disabled with a short explanation. */}
        <div style={{ marginTop: 12 }}>
          <label
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              fontSize: 13,
              color: permSupported ? "var(--text-primary)" : "var(--text-muted)",
              cursor: permSupported ? "pointer" : "default",
            }}
          >
            <input
              type="checkbox"
              disabled={!permSupported}
              checked={permSupported && skipPerm}
              onChange={(e) => setSkipPerm(e.target.checked)}
            />
            {t("tree.permissionSkipLabel")}
          </label>
          <div
            style={{
              fontSize: 11,
              color: "var(--text-muted)",
              lineHeight: 1.5,
              marginTop: 4,
              marginLeft: 24,
            }}
          >
            {permSupported
              ? t("tree.permissionSkipHint")
              : kind === "pi"
                ? t("tree.permissionUnsupportedPi")
                : t("tree.permissionUnsupported")}
          </div>
        </div>

        <div
          style={{
            display: "flex",
            justifyContent: "flex-end",
            gap: 8,
            marginTop: 18,
          }}
        >
          <button className="vlx-btn" onClick={onCancel}>
            {t("common.cancel")}
          </button>
          <button
            className="vlx-btn vlx-btn-primary"
            onClick={submit}
            disabled={!canSubmit}
          >
            {t("resume.confirm")}
          </button>
        </div>
      </div>
    </Backdrop>
  );
}
