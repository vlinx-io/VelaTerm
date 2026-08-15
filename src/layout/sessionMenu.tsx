//! Shared session context menus and dialogs for the sidebar tree and center tab bar. `useSessionMenu()` returns new
//! session items, per-session actions, a dialog opener, and dialog JSX. Centralizing former LeftSidebar logic prevents
//! tab and tree menus from drifting apart.

import { useEffect, useState } from "react";
import { type MenuItem } from "../components/ContextMenu";
import { FormModal, type FieldDef } from "../components/FormModal";
import Icons from "../components/Icons";
import { canExportContext, exportSessionToFile } from "../exportSession";
import { t, useT } from "../i18n";
import {
  createWorktree,
  downloadFullGitbash,
  gitbashStatus,
  type GitbashStatus,
  listShells,
  ptyKill,
  removeWorktree,
  type ShellOption,
  worktreesInSubtree,
} from "../ipc/commands";
import { copyText, openDir } from "../ipc/info";
import { onGitbashDownloadDone } from "../ipc/events";
import { env } from "../platform";
import { DOC_SAVE_EVENT } from "../hooks/useKeyboardShortcuts";
import {
  invalidateGitBranch,
  isWorktreeGone,
  peekGitBranchInfo,
  prefetchGitBranchInfo,
} from "../hooks/useGitBranch";
import { type SelNode, useTermStore } from "../store/termStore";
import {
  type NodeKind,
  type Session,
  type SessionKind,
  supportsPermissionToggle,
} from "../types";
import { MARK_LABEL_KEYS, NODE_MARKS, normalizeMark } from "../marks";
import { type TreeNodeRef } from "./LeftSidebar/ProjectTree";
import { kindIconEl } from "./sessionViewers/sessionMeta";
import {
  ArchiveBlocked,
  archiveErrorText,
  ConfirmArchive,
  ConfirmDelete,
  DeleteWorktree,
  NewAgentSession,
  GroupInfo,
  NewGroup,
  ResumeSession,
  SessionInfo,
} from "./sessionMenuDialogs";
import { archivedWorktreePaths } from "./archivePlan";
import { kindLabel, supportsAgentArgs } from "./sessionMenuShared";

/** Number of direct agent shortcuts on the first New Session menu level. */
const QUICK_AGENT_COUNT = 3;
/** Default shortcut order when no recent history exists. */
const DEFAULT_QUICK_AGENTS: SessionKind[] = ["claude", "codex", "opencode"];
/** Local agent kinds eligible for one-click first-level shortcuts. Separate from AGENT_ARGS_KINDS despite overlap. */
const QUICK_AGENT_KINDS: SessionKind[] = [
  "claude",
  "codex",
  "opencode",
  "copilot",
  "cursor",
  "antigravity",
  "cline",
  "pi",
  "crush",
  "kimi",
  "kiro",
  "grok",
  "zoo",
];

/**
 * Chooses three direct New Session agent types from globally recent creations, deduplicated and filled from the
 * default Claude/Codex/OpenCode order. Frequently used types naturally rise to the first level.
 */
function computeQuickAgents(sessions: Session[]): SessionKind[] {
  const recent = sessions
    .filter((s) => QUICK_AGENT_KINDS.includes(s.kind))
    .slice()
    .sort((a, b) => b.createdAt - a.createdAt)
    .map((s) => s.kind);
  const ordered: SessionKind[] = [];
  for (const k of [...recent, ...DEFAULT_QUICK_AGENTS]) {
    if (!ordered.includes(k)) ordered.push(k);
    if (ordered.length === QUICK_AGENT_COUNT) break;
  }
  return ordered;
}

export type Dialog =
  | { type: "newGroup"; projectId: string; parentGroupId: string | null }
  | {
      // Custom-argument and worktree creation share a dialog for agent kind, optional name/arguments, and worktree
      // mode (none/new/existing). Normal creation remains immediate. initialWtMode selects the opening worktree tab.
      type: "newAgentSession";
      projectId: string;
      groupId: string | null;
      parentSessionId: string | null;
      initialWtMode?: "none" | "new" | "existing";
    }
  | {
      type: "resumeSession";
      projectId: string;
      groupId: string | null;
      parentSessionId: string | null;
    }
  | { type: "editSession"; id: string; initial: Record<string, string> }
  | { type: "sessionInfo"; id: string; advanced?: boolean }
  | { type: "groupInfo"; id: string }
  | { type: "archiveBlocked"; message: string }
  | {
      type: "confirmArchive";
      target: "sessions" | "group";
      ids: string[];
      worktreePaths: string[];
    }
  | { type: "confirmDelete"; node: TreeNodeRef; worktreePaths: string[] }
  | { type: "confirmDeleteMany"; nodes: SelNode[] }
  | {
      // Delete Worktree lists repository worktrees; defaultPath preselects the triggering node's binding when present.
      type: "deleteWorktree";
      repoRoot: string;
      defaultPath: string | null;
    };

export interface SessionMenu {
  /** New Session items. withTerminal adds a terminal entry for sidebar/group menus; withBrowser adds a browser page
   *  only for project/group menus. */
  newSessionItems: (
    projectId: string,
    groupId: string | null,
    parentSessionId?: string | null,
    opts?: { withTerminal?: boolean; withBrowser?: boolean },
  ) => MenuItem[];
  /** Per-session actions. Rename appears only with onRename; sidebar uses inline rename while tabs use Edit.
   *  statusViewId adds Refresh Status, which re-checks this session against that sidebar view's status filter;
   *  it is omitted for tabs and for views without an active status filter. */
  buildSessionItems: (
    node: TreeNodeRef,
    opts?: { onRename?: () => void; statusViewId?: string | null },
  ) => MenuItem[];
  /** Reduced menu for ephemeral drafts, omitting persistence-only delete/archive/fork/export/move actions. Options
   *  customize rename/close, hide Open for tabs, or omit duplicate Close where the tab already supplies one. */
  buildScratchItems: (
    node: TreeNodeRef,
    opts?: {
      variant?: "terminal" | "browser" | "doc";
      onRename?: () => void;
      onClose?: () => void;
      showOpen?: boolean;
      omitClose?: boolean;
    },
  ) => MenuItem[];
  /** Shell-switch items for persistent or draft terminals; empty for non-terminals or when no shells are detected. */
  shellSwitchItems: (sessionId: string) => MenuItem[];
  /** Batch Move Selected To menu using moveMany. Available only for nonempty selections within one project. */
  buildMoveToMany: (sessionIds: string[]) => MenuItem | null;
  /** Mark submenu offering the emoji palette plus No Mark, checking the node's current marker. Works for any node
   *  kind, so the sidebar reuses it for projects and groups. */
  buildMarkItem: (kind: NodeKind, id: string) => MenuItem;
  /** Shared Git submenu: repositories get View Changes/Merge; worktrees also get Copy Path/Open Directory. Empty for
   *  non-repositories. MergeModal resolves either a session or group mergeTargetId. */
  buildGitItems: (opts: {
    repoDir: string | null;
    worktreePath?: string | null;
    mergeTargetId: string;
  }) => MenuItem[];
  /** Opens a dialog, including batch and project/group deletion confirmation. */
  openDialog: (dialog: Dialog) => void;
  /** Dialog rendering owned independently by each sidebar/tab-bar hook instance. */
  dialogs: React.ReactNode;
}

/**
 * Shared menu/dialog hook. Sidebar and tab bar each own independent dialog state and render their own `dialogs`.
 */
export function useSessionMenu(): SessionMenu {
  const t = useT();
  const addSession = useTermStore((s) => s.addSession);
  const openSession = useTermStore((s) => s.openSession);
  // New Terminal creates an eph- draft only in the center pane, without persistence or a tree node.
  const newScratchTab = useTermStore((s) => s.newScratchTab);
  const openMerge = useTermStore((s) => s.openMerge);
  const openChanges = useTermStore((s) => s.openChanges);
  const updateSession = useTermStore((s) => s.updateSession);
  const deleteNode = useTermStore((s) => s.deleteNode);
  const deleteMany = useTermStore((s) => s.deleteMany);
  const moveNode = useTermStore((s) => s.moveNode);
  const moveMany = useTermStore((s) => s.moveMany);
  const archiveSession = useTermStore((s) => s.archiveSession);
  const archiveMany = useTermStore((s) => s.archiveMany);
  const archiveGroup = useTermStore((s) => s.archiveGroup);
  const setNodeMark = useTermStore((s) => s.setNodeMark);
  const clearNodeWorktree = useTermStore((s) => s.clearNodeWorktree);
  const forkSession = useTermStore((s) => s.forkSession);
  const closeSession = useTermStore((s) => s.closeSession);
  const setActiveTab = useTermStore((s) => s.setActiveTab);
  const closeTab = useTermStore((s) => s.closeTab);
  const requestCloseDocTab = useTermStore((s) => s.requestCloseDocTab);
  const addGroup = useTermStore((s) => s.addGroup);
  const groups = useTermStore((s) => s.groups);
  const sessions = useTermStore((s) => s.sessions);

  const [dialog, setDialog] = useState<Dialog | null>(null);
  // Detect platform shells once for selectors. Empty/legacy results use a text input and omit quick switching.
  const [shells, setShells] = useState<ShellOption[]>([]);
  // Windows Git Bash status controls whether Download Full Git Bash appears in the Shell submenu.
  const [gitbash, setGitbash] = useState<GitbashStatus | null>(null);
  useEffect(() => {
    let alive = true;
    const refresh = () => {
      listShells()
        .then((s) => alive && setShells(s))
        .catch(() => {});
      gitbashStatus()
        .then((g) => alive && setGitbash(g))
        .catch(() => {});
    };
    refresh();
    // After full download, remove the action and refresh shells to the full path.
    const un = onGitbashDownloadDone(() => refresh());
    return () => {
      alive = false;
      void un.then((fn) => fn());
    };
  }, []);

  // switchSessionShell handles persisted/draft updates and restarts running sessions, shared with the inline selector.
  const switchShell = (id: string, shellPath: string) =>
    useTermStore.getState().switchSessionShell(id, shellPath);

  // Quick Shell items appear only for terminals with detected shells and mark the current choice.
  const shellSwitchItems = (id: string): MenuItem[] => {
    const st = useTermStore.getState();
    const sess = st.sessions.find((x) => x.id === id) ?? st.ephemeralSessions[id];
    if (!sess || sess.kind !== "terminal" || !shells.length) return [];
    const submenu: MenuItem[] = shells.map((sh) => ({
      label: (sess.shell === sh.path ? "✓ " : "　") + sh.label,
      onClick: () => void switchShell(id, sh.path),
    }));
    // On Windows, offer one-click full Git Bash when only the bundled minimal distribution is available.
    if (gitbash?.available && !gitbash.fullInstalled) {
      submenu.push({ label: "", separator: true });
      submenu.push({
        label: t("tree.downloadFullGitbash"),
        onClick: () => void downloadFullGitbash(),
      });
    }
    return [{ label: t("tree.shellMenu"), submenu }];
  };

  // Mark submenu. The label carries the emoji so the palette is readable at a glance, and the current marker is
  // checked exactly like the Shell submenu. Picking the marker that is already set clears it, mirroring the sidebar
  // marker filter, so no separate "no mark" entry is needed. Read the node from the store on demand because the same
  // builder serves projects, groups, and sessions.
  const buildMarkItem = (kind: NodeKind, id: string): MenuItem => {
    const st = useTermStore.getState();
    const node =
      kind === "project"
        ? st.projects.find((p) => p.id === id)
        : kind === "group"
          ? st.groups.find((g) => g.id === id)
          : st.sessions.find((s) => s.id === id);
    const current = normalizeMark(node?.mark);
    const check = (on: boolean) => (on ? "✓ " : "　");
    const submenu: MenuItem[] = NODE_MARKS.map((m) => ({
      label: `${check(current === m)}${m}  ${t(MARK_LABEL_KEYS[m])}`,
      onClick: () => void setNodeMark(kind, id, current === m ? null : m),
    }));
    return { label: t("mark.menu"), submenu };
  };

  // Quick create/resume inherits a parent group's worktree into cwd/path/baseRef for local sessions. Browser nodes
  // do not inherit, and explicit custom-dialog worktree choices always take precedence.
  const groupWorktreeDefault = (
    groupId: string | null,
    kind: SessionKind,
  ): { cwd: string; worktreePath: string; worktreeBaseRef: string | null } | null => {
    if (!groupId || kind === "browser") return null;
    const g = useTermStore.getState().groups.find((x) => x.id === groupId);
    if (!g?.worktreePath) return null;
    return {
      cwd: g.worktreePath,
      worktreePath: g.worktreePath,
      worktreeBaseRef: g.worktreeBaseRef ?? null,
    };
  };

  // Create/open immediately after kind selection, using an explicit name or generated Kind N and persisting agent arguments.
  const handleNewSession = async (
    projectId: string,
    groupId: string | null,
    kind: SessionKind,
    parentSessionId: string | null = null,
    // Terminal-only explicit shell; empty means system default.
    shell: string | null = null,
    opts?: { name?: string; agentArgs?: string; permissionMode?: string | null },
  ) => {
    const st = useTermStore.getState();
    // Include ephemeral terminals when numbering so menu and tab/Cmd+T entry points do not collide.
    const all = [...st.sessions, ...Object.values(st.ephemeralSessions)];
    const def = st.agentDefaults[kind] ?? {};
    const label =
      kind === "claude"
        ? "Claude"
        : kind === "codex"
          ? "Codex"
          : kind === "opencode"
            ? "OpenCode"
            : kind === "copilot"
              ? "Copilot"
              : kind === "cursor"
                ? "Cursor"
                : kind === "antigravity"
                  ? "Antigravity"
                  : kind === "cline"
                    ? "Cline"
                    : kind === "pi"
                      ? "Pi"
                      : kind === "crush"
                        ? "Crush"
                        : kind === "kimi"
                          ? "Kimi Code"
                          : kind === "kiro"
                            ? "Kiro"
                          : kind === "grok"
                            ? "Grok Build"
                          : kind === "zoo"
                            ? "Zoo Code"
                        : kind === "browser"
                          ? t("kind.browser")
                        : t("kind.terminal");
    // Use the maximum existing same-kind suffix plus one; total counts regress after deletion and can collide.
    const re = new RegExp(`^${label} (\\d+)$`);
    const maxN = all
      .filter((s) => s.projectId === projectId && s.kind === kind)
      .reduce((m, s) => {
        const mt = re.exec(s.name);
        return mt ? Math.max(m, Number(mt[1])) : m;
      }, 0);
    const n = maxN + 1;
    const customName = opts?.name?.trim();
    // Explicit agentArgs/permissionMode, including empty/null, represent user choices. One-click creation omits them
    // and inherits the agent kind's global defaults.
    const rawArgs = opts?.agentArgs !== undefined ? opts.agentArgs : def.args ?? "";
    const rawPerm =
      opts?.permissionMode !== undefined ? opts.permissionMode : def.permissionMode ?? null;
    // Inherit a group's cwd/worktreePath/baseRef when present.
    const wt = groupWorktreeDefault(groupId, kind);
    const created = await addSession({
      projectId,
      groupId,
      name: customName || `${label} ${n}`,
      kind,
      parentSessionId,
      shell: shell || undefined,
      agentArgs: rawArgs.trim() || null,
      permissionMode: rawPerm || null,
      ...(wt
        ? { cwd: wt.cwd, worktreePath: wt.worktreePath, worktreeBaseRef: wt.worktreeBaseRef }
        : {}),
    });
    if (created) openSession(created.id);
  };


  // Resume creates a new node prepopulated with an agent-session anchor, then uses the existing as-is resume flow.
  // copilot --resume= / cursor-agent --resume= / cline --id）。
  // Dialog-provided agentArgs/permissionMode start from global defaults but may be changed/cleared and persist across resumes.
  const handleResumeSession = async (
    projectId: string,
    groupId: string | null,
    parentSessionId: string | null,
    kind: SessionKind,
    agentSessionId: string,
    nameInput: string,
    opts: { agentArgs: string; permissionMode: string | null },
  ) => {
    const sid = agentSessionId.trim();
    if (!sid) return;
    let name = nameInput.trim();
    if (!name) {
      // Empty name uses the same per-project, per-kind maximum suffix plus one.
      const all = useTermStore.getState().sessions;
      const label = kindLabel(kind);
      const re = new RegExp(`^${label} (\\d+)$`);
      const maxN = all
        .filter((s) => s.projectId === projectId && s.kind === kind)
        .reduce((m, s) => {
          const mt = re.exec(s.name);
          return mt ? Math.max(m, Number(mt[1])) : m;
        }, 0);
      name = `${label} ${maxN + 1}`;
    }
    // Resumed sessions inherit a group worktree just like new sessions.
    const wt = groupWorktreeDefault(groupId, kind);
    const created = await addSession({
      projectId,
      groupId,
      name,
      kind,
      parentSessionId,
      agentSessionId: sid,
      agentArgs: opts.agentArgs.trim() || null,
      permissionMode: opts.permissionMode || null,
      ...(wt
        ? { cwd: wt.cwd, worktreePath: wt.worktreePath, worktreeBaseRef: wt.worktreeBaseRef }
        : {}),
    });
    if (created) openSession(created.id);
  };

  // New worktree session first creates an isolated worktree in the parent/project repository, then launches there.
  // Errors are shown by the dialog and no session is created.
  const handleNewWorktreeSession = async (
    projectId: string,
    groupId: string | null,
    parentSessionId: string | null,
    payload: {
      kind: SessionKind;
      name: string;
      worktreeName: string;
      agentArgs: string;
      permissionMode: string | null;
    },
  ) => {
    const st = useTermStore.getState();
    const parent = parentSessionId
      ? st.sessions.find((s) => s.id === parentSessionId)
      : null;
    const project = st.projects.find((p) => p.id === projectId);
    const repoRoot = parent?.cwd || project?.rootPath || null;
    if (!repoRoot) throw new Error(t("worktree.noRepoRoot"));
    // Worktree directory and branch use a dedicated worktree name independent of the session name.
    const wt = await createWorktree(repoRoot, payload.worktreeName);
    const created = await addSession({
      projectId,
      groupId,
      name: payload.name,
      kind: payload.kind,
      cwd: wt.path,
      parentSessionId,
      worktreePath: wt.path,
      worktreeBaseRef: wt.baseRef || null,
      agentArgs: payload.agentArgs.trim() || null,
      permissionMode: payload.permissionMode || null,
    });
    if (created) openSession(created.id);
  };

  // Existing-worktree creation points cwd/path at the selection. Its historical base cannot be reconstructed, so
  // baseRef remains empty and landing falls back to the main worktree's current branch.
  const handleNewSessionInWorktree = async (
    projectId: string,
    groupId: string | null,
    parentSessionId: string | null,
    payload: {
      kind: SessionKind;
      name: string;
      worktreePath: string;
      agentArgs: string;
      permissionMode: string | null;
    },
  ) => {
    const created = await addSession({
      projectId,
      groupId,
      name: payload.name,
      kind: payload.kind,
      cwd: payload.worktreePath,
      parentSessionId,
      worktreePath: payload.worktreePath,
      worktreeBaseRef: null,
      agentArgs: payload.agentArgs.trim() || null,
      permissionMode: payload.permissionMode || null,
    });
    if (created) openSession(created.id);
  };

  // New Terminal creates a center-only eph- draft, never a tree child. Multiple detected shells produce a submenu;
  // otherwise create directly with the configured default, matching tab plus/Cmd+T.
  const newTerminalItem = (
    projectId: string,
    groupId: string | null,
    parentSessionId: string | null,
  ): MenuItem => {
    const target = { projectId, groupId, sessionId: parentSessionId };
    return shells.length
      ? {
          label: t("tree.newTerminalSession"),
          icon: kindIconEl("terminal"),
          submenu: [
            {
              label: t("tree.shellSystemDefault"),
              onClick: () => void newScratchTab({ shell: null, target }),
            },
            ...shells.map((sh) => ({
              label: sh.label,
              onClick: () => void newScratchTab({ shell: sh.path, target }),
            })),
          ],
        }
      : {
          label: t("tree.newTerminalSession"),
          icon: kindIconEl("terminal"),
          onClick: () => void newScratchTab({ target }),
        };
  };

  // New Session can include browser, terminal, agent submenu, and resume. withTerminal is used by sidebar plus/group
  // menus but not child-session menus. withBrowser creates a persistent Browser node only for project/group menus and
  // is hidden remotely because no native child WebView exists.
  const newSessionItems = (
    projectId: string,
    groupId: string | null,
    parentSessionId: string | null = null,
    opts?: { withTerminal?: boolean; withBrowser?: boolean },
  ): MenuItem[] => [
    ...(opts?.withBrowser && (env.isTauri || env.isElectron)
      ? [
          {
            label: t("tree.newBrowserPage"),
            icon: kindIconEl("browser"),
            onClick: () => void handleNewSession(projectId, groupId, "browser"),
          },
        ]
      : []),
    ...(opts?.withTerminal ? [newTerminalItem(projectId, groupId, parentSessionId)] : []),
    // Show three recent/default direct agent shortcuts; the complete set, remote, and custom creation remain nested.
    ...computeQuickAgents(sessions).map((k) => ({
      label: t("tree.newAgentSession", kindLabel(k)),
      icon: kindIconEl(k),
      onClick: () => void handleNewSession(projectId, groupId, k, parentSessionId),
    })),
    // Group the full agent list under New Agent Session rather than occupying most of the top-level menu.
    {
      label: t("tree.newAgentSessionGroup"),
      icon: <Icons.bot size={14} />,
      submenu: [
        // Local agents create immediately through the default fast path.
        {
          label: t("tree.newAgentSession", "Claude"),
          icon: kindIconEl("claude"),
          onClick: () => void handleNewSession(projectId, groupId, "claude", parentSessionId),
        },
        {
          label: t("tree.newAgentSession", "Codex"),
          icon: kindIconEl("codex"),
          onClick: () => void handleNewSession(projectId, groupId, "codex", parentSessionId),
        },
        {
          label: t("tree.newAgentSession", "OpenCode"),
          icon: kindIconEl("opencode"),
          onClick: () => void handleNewSession(projectId, groupId, "opencode", parentSessionId),
        },
        {
          label: t("tree.newAgentSession", "Copilot"),
          icon: kindIconEl("copilot"),
          onClick: () =>
            void handleNewSession(projectId, groupId, "copilot", parentSessionId),
        },
        {
          label: t("tree.newAgentSession", "Cursor"),
          icon: kindIconEl("cursor"),
          onClick: () =>
            void handleNewSession(projectId, groupId, "cursor", parentSessionId),
        },
        {
          label: t("tree.newAgentSession", "Antigravity"),
          icon: kindIconEl("antigravity"),
          onClick: () =>
            void handleNewSession(projectId, groupId, "antigravity", parentSessionId),
        },
        {
          label: t("tree.newAgentSession", "Cline"),
          icon: kindIconEl("cline"),
          onClick: () =>
            void handleNewSession(projectId, groupId, "cline", parentSessionId),
        },
        {
          label: t("tree.newAgentSession", "Pi"),
          icon: kindIconEl("pi"),
          onClick: () => void handleNewSession(projectId, groupId, "pi", parentSessionId),
        },
        {
          label: t("tree.newAgentSession", "Crush"),
          icon: kindIconEl("crush"),
          onClick: () => void handleNewSession(projectId, groupId, "crush", parentSessionId),
        },
        {
          label: t("tree.newAgentSession", "Kimi Code (K3)"),
          icon: kindIconEl("kimi"),
          onClick: () => void handleNewSession(projectId, groupId, "kimi", parentSessionId),
        },
        {
          label: t("tree.newAgentSession", "Kiro"),
          icon: kindIconEl("kiro"),
          onClick: () => void handleNewSession(projectId, groupId, "kiro", parentSessionId),
        },
        {
          label: t("tree.newAgentSession", "Grok Build (Grok 4.5)"),
          icon: kindIconEl("grok"),
          onClick: () => void handleNewSession(projectId, groupId, "grok", parentSessionId),
        },
        {
          label: t("tree.newAgentSession", "Zoo Code"),
          icon: kindIconEl("zoo"),
          onClick: () => void handleNewSession(projectId, groupId, "zoo", parentSessionId),
        },
        // Custom launch arguments use the dialog for kind, optional name, and arguments.
        { label: "", separator: true },
        {
          label: t("tree.newAgentSessionCustom"),
          icon: <Icons.sliders size={14} />,
          onClick: () =>
            setDialog({ type: "newAgentSession", projectId, groupId, parentSessionId }),
        },
      ],
    },
    // New Worktree Session shares the custom dialog but opens in new-worktree mode; users can change to existing/none.
    {
      label: t("tree.newWorktreeSession"),
      icon: <Icons.branch size={14} />,
      onClick: () =>
        setDialog({
          type: "newAgentSession",
          projectId,
          groupId,
          parentSessionId,
          initialWtMode: "new",
        }),
    },
    // Resume remains adjacent to creation actions in the Session section.
    {
      label: t("tree.resumeSession"),
      icon: <Icons.restart size={14} />,
      onClick: () =>
        setDialog({ type: "resumeSession", projectId, groupId, parentSessionId }),
    },
  ];

  // Git submenu offers View Changes/Merge for repositories and Copy Path/Open Directory for worktrees. Hide known
  // non-repositories; optimistically show unknown paths while warming cache. Worktree nodes skip detection.
  const buildGitItems = (opts: {
    repoDir: string | null;
    worktreePath?: string | null;
    mergeTargetId: string;
  }): MenuItem[] => {
    const { repoDir, worktreePath, mergeTargetId } = opts;
    if (!repoDir) return [];
    if (!worktreePath) {
      const info = peekGitBranchInfo(repoDir);
      if (info === undefined) prefetchGitBranchInfo(repoDir);
      else if (!info.isRepo) return []; // Known non-repository.
    }
    const sep: MenuItem = { label: "", separator: true };
    return [
      {
        label: t("tree.gitMenu"),
        icon: <Icons.branch size={14} />,
        submenu: [
          { label: t("tree.viewChanges"), onClick: () => openChanges(repoDir) },
          { label: t("tree.merge"), onClick: () => openMerge(mergeTargetId) },
          ...(worktreePath
            ? [
                sep,
                {
                  label: t("tree.copyWorktreePath"),
                  onClick: () => void copyText(worktreePath),
                },
                {
                  label: t("tree.openWorktreeDir"),
                  onClick: () => void openDir(worktreePath),
                },
              ]
            : []),
          // Any repository node can open Delete Worktree; preselect its binding or leave selection empty.
          sep,
          {
            label: t("tree.deleteWorktreeMenu"),
            danger: true,
            onClick: () =>
              setDialog({
                type: "deleteWorktree",
                repoRoot: repoDir,
                defaultPath: worktreePath ?? null,
              }),
          },
        ],
      },
    ];
  };

  // Per-session actions shared by tree nodes and tabs.
  const buildSessionItems = (
    node: TreeNodeRef,
    opts?: { onRename?: () => void; statusViewId?: string | null },
  ): MenuItem[] => {
    const sep: MenuItem = { label: "", separator: true };
    // Refresh Status re-checks only this row against the pane's status filter, so a session that finished working
    // leaves an Awaiting/Working view without refreshing every other row.
    const statusView = opts?.statusViewId
      ? useTermStore.getState().sidebarTreeViews.find((v) => v.id === opts.statusViewId)
      : undefined;
    const refreshStatusItem: MenuItem | null =
      statusView?.statusFilter
        ? {
            label: t("tree.refreshStatusMatch"),
            icon: <Icons.restart size={14} />,
            onClick: () =>
              useTermStore
                .getState()
                .refreshSidebarTreeViewStatusMatch(statusView.id, node.id),
          }
        : null;
    const renameItem: MenuItem | null = opts?.onRename
      ? { label: t("common.rename"), onClick: opts.onRename }
      : null;
    const remove: MenuItem = {
      label: t("tree.deleteSession"),
      danger: true,
      onClick: () => {
        void worktreesInSubtree(node.id)
          .then((paths) => setDialog({ type: "confirmDelete", node, worktreePaths: paths }))
          .catch(() => setDialog({ type: "confirmDelete", node, worktreePaths: [] }));
      },
    };

    const buildMoveTo = (): MenuItem => {
      const inProject = groups.filter((g) => g.projectId === node.projectId);
      const flat: { id: string; label: string }[] = [];
      const walk = (parentGroupId: string | null, depth: number) => {
        for (const g of inProject.filter((g) => (g.parentGroupId ?? null) === parentGroupId)) {
          flat.push({ id: g.id, label: "　".repeat(depth) + g.name });
          walk(g.id, depth + 1);
        }
      };
      walk(null, 0);
      const rootItem: MenuItem = {
        label: t("tree.projectRoot"),
        icon: <Icons.project size={14} />,
        disabled: node.groupId === null,
        onClick: () => void moveNode("session", node.id, node.projectId, null, null, Date.now()),
      };
      const groupItems: MenuItem[] = flat.map((g) => ({
        label: g.label,
        icon: <Icons.folder size={14} />,
        disabled: node.groupId === g.id,
        onClick: () => void moveNode("session", node.id, node.projectId, g.id, null, Date.now()),
      }));

      // Session targets nest this session like dropping on a session. Exclude self and its subtree to prevent cycles;
      // disable the current parent.
      const self = sessions.find((s) => s.id === node.id);
      const inProjectSessions = sessions.filter((s) => s.projectId === node.projectId);
      const sessFlat: { id: string; groupId: string | null; kind: SessionKind; label: string }[] =
        [];
      const sessWalk = (parentSessionId: string | null, depth: number) => {
        for (const s of inProjectSessions.filter(
          (s) => (s.parentSessionId ?? null) === parentSessionId,
        )) {
          if (s.id === node.id) continue; // Skip self and the entire subtree.
          sessFlat.push({
            id: s.id,
            groupId: s.groupId ?? null,
            kind: s.kind,
            label: "　".repeat(depth) + s.name,
          });
          sessWalk(s.id, depth + 1);
        }
      };
      sessWalk(null, 0);
      // Moving under a session adopts the target's group, matching dropOnSession.
      const sessionItems: MenuItem[] = sessFlat.map((s) => ({
        label: s.label,
        icon: kindIconEl(s.kind, 14),
        disabled: self?.parentSessionId === s.id,
        onClick: () =>
          void moveNode("session", node.id, node.projectId, s.groupId, s.id, Date.now()),
      }));

      const submenu: MenuItem[] = [rootItem, ...groupItems];
      if (sessionItems.length > 0) {
        submenu.push(
          { label: "", separator: true },
          { label: t("tree.moveToSession"), disabled: true },
          ...sessionItems,
        );
      }
      return { label: t("tree.moveTo"), submenu };
    };

    const sessionRec = sessions.find((s) => s.id === node.id);
    // Browser nodes keep only Open, optional Rename, Move, and Delete; PTY/agent-specific actions do not apply.
    if (sessionRec?.kind === "browser") {
      const items: MenuItem[] = [
        { label: t("common.open"), onClick: () => openSession(node.id) },
        sep,
      ];
      if (renameItem) items.push(renameItem, sep);
      if (refreshStatusItem) items.push(refreshStatusItem);
      items.push(buildMarkItem("session", node.id), sep, buildMoveTo(), sep, remove);
      return items;
    }
    // Fork is available only to Claude/Codex/Pi with a captured conversation ID and native fork support.
    const canFork =
      (sessionRec?.kind === "claude" ||
        sessionRec?.kind === "codex" ||
        sessionRec?.kind === "pi") &&
      !!sessionRec?.agentSessionId;

    // Quick shell switching applies only to plain terminals with detected shells; agents use fixed injection logic.
    const shellMenu = shellSwitchItems(node.id);

    // Git directory priority is worktree, session cwd, then project root for inherited empty cwd.
    const gitRepoDir =
      sessionRec?.worktreePath ||
      (sessionRec?.cwd ?? "").trim() ||
      (useTermStore.getState().projects.find((p) => p.id === node.projectId)?.rootPath ?? "").trim() ||
      null;

    const items: MenuItem[] = [
      { label: t("common.open"), onClick: () => openSession(node.id) },
      { label: t("tree.openNewTab"), onClick: () => openSession(node.id, { newTab: true }) },
      { label: t("tree.newChildSession"), submenu: newSessionItems(node.projectId, node.groupId, node.id) },
      ...shellMenu,
      ...(canFork
        ? [{ label: t("tree.forkSession"), onClick: () => void forkSession(node.id) }]
        : []),
      // Export context for local Claude/Codex sessions with captured parseable conversation IDs.
      ...(sessionRec && canExportContext(sessionRec)
        ? [{ label: t("tree.exportSession"), onClick: () => void exportSessionToFile(sessionRec) }]
        : []),
      // Show the Git submenu for any session whose working directory is a repository, not just worktrees.
      ...buildGitItems({
        repoDir: gitRepoDir,
        worktreePath: sessionRec?.worktreePath,
        mergeTargetId: node.id,
      }),
      // Convert to Normal appears only after the bound worktree directory is deleted; it clears bindings/cwd so
      // launches return to project root. Worktree deletion and conversion are separate actions.
      ...(isWorktreeGone(sessionRec?.worktreePath)
        ? [
            {
              label: t("tree.convertToNormalSession"),
              onClick: () => void clearNodeWorktree("session", node.id),
            },
          ]
        : []),
      sep,
      ...(refreshStatusItem ? [refreshStatusItem] : []),
      // Marks apply only to persisted nodes; drafts have no database row to write.
      buildMarkItem("session", node.id),
      {
        label: t("tree.sessionInfo"),
        // Option/Alt opens advanced mode with the complete launch command.
        onClick: (e) => setDialog({ type: "sessionInfo", id: node.id, advanced: e.altKey }),
      },
      {
        label: t("common.edit"),
        onClick: () => {
          const s = useTermStore.getState().sessions.find((x) => x.id === node.id);
          setDialog({
            type: "editSession",
            id: node.id,
            initial: {
              name: s?.name ?? node.name,
              shell: s?.shell ?? "",
              cwd: s?.cwd ?? "",
              initCmd: s?.initCmd ?? "",
              agentArgs: s?.agentArgs ?? "",
              // Two-state toggle: checked maps to "skip"; empty/legacy values are unchecked.
              permissionMode: s?.permissionMode === "skip" ? "skip" : "",
            },
          });
        },
      },
    ];
    if (renameItem) items.push(renameItem);
    items.push(sep, buildMoveTo(), sep);
    // End Process appears only while running. Browser tab closure merely detaches, so this is required to stop
    // headless processes. Local kill broadcasts are filtered, so close this view directly while others close by event.
    if (useTermStore.getState().runtimes[node.id]?.status === "running") {
      items.push({
        label: t("tree.killProcess"),
        onClick: () => {
          void ptyKill(node.id).catch(() => {});
          useTermStore.getState().closeSession(node.id);
        },
      });
    }
    items.push(
      {
        label: t("tree.archiveSession"),
        onClick: () => {
          // Archiving retires every worker worktree in the subtree, which cannot be undone.
          const worktreePaths = archivedWorktreePaths(sessions, groups, [node.id]);
          if (worktreePaths.length > 0) {
            setDialog({
              type: "confirmArchive",
              target: "sessions",
              ids: [node.id],
              worktreePaths,
            });
            return;
          }
          void archiveSession(node.id).catch((e: unknown) =>
            setDialog({ type: "archiveBlocked", message: archiveErrorText(e) }),
          );
        },
      },
      remove,
    );
    return items;
  };

  // Batch Move Selected To uses one moveMany call for a same-project selection. Cross-project moves are ambiguous;
  // targets already containing every selected session are disabled.
  const buildMoveToMany = (sessionIds: string[]): MenuItem | null => {
    const recs = sessionIds
      .map((id) => sessions.find((s) => s.id === id))
      .filter((s): s is Session => !!s);
    if (recs.length === 0) return null;
    const projectId = recs[0].projectId;
    if (!recs.every((s) => s.projectId === projectId)) return null; // Mixed-project selection is unsupported.

    const ids = recs.map((s) => s.id);
    const inProject = groups.filter((g) => g.projectId === projectId);
    const flat: { id: string; label: string }[] = [];
    const walk = (parentGroupId: string | null, depth: number) => {
      for (const g of inProject.filter((g) => (g.parentGroupId ?? null) === parentGroupId)) {
        flat.push({ id: g.id, label: "　".repeat(depth) + g.name });
        walk(g.id, depth + 1);
      }
    };
    walk(null, 0);

    const rootItem: MenuItem = {
      label: t("tree.projectRoot"),
      icon: <Icons.project size={14} />,
      // Disable project root when every session is already ungrouped there.
      disabled: recs.every((s) => (s.groupId ?? null) === null),
      onClick: () => void moveMany(ids, projectId, null, null),
    };
    const groupItems: MenuItem[] = flat.map((g) => ({
      label: g.label,
      icon: <Icons.folder size={14} />,
      // Disable a group that already contains every selected session.
      disabled: recs.every((s) => (s.groupId ?? null) === g.id),
      onClick: () => void moveMany(ids, projectId, g.id, null),
    }));

    return { label: t("tree.moveSelected"), submenu: [rootItem, ...groupItems] };
  };

  // Reduced draft menu for temporary terminal sessions, browser tabs, and document tabs.
  const buildScratchItems = (
    node: TreeNodeRef,
    opts?: {
      variant?: "terminal" | "browser" | "doc";
      onRename?: () => void;
      onClose?: () => void;
      showOpen?: boolean;
      omitClose?: boolean;
    },
  ): MenuItem[] => {
    const sep: MenuItem = { label: "", separator: true };
    const variant = opts?.variant ?? "terminal";
    const showOpen = opts?.showOpen ?? true;
    const items: MenuItem[] = [];

    // Open uses openSession for terminals; browser/doc drafts simply focus their tab.
    if (showOpen) {
      if (variant === "terminal") {
        items.push(
          { label: t("common.open"), onClick: () => openSession(node.id) },
          { label: t("tree.openNewTab"), onClick: () => openSession(node.id, { newTab: true }) },
        );
      } else {
        items.push({ label: t("common.open"), onClick: () => setActiveTab(node.id) });
      }
      items.push(sep);
    }

    // Convert to Permanent was removed for center-only terminal/browser drafts. Document Save persists content and remains.
    if (variant === "doc") {
      items.push({
        label: t("tree.persistDoc"),
        onClick: () =>
          window.dispatchEvent(new CustomEvent(DOC_SAVE_EVENT, { detail: node.id })),
      });
    }

    if (opts?.onRename) items.push({ label: t("common.rename"), onClick: opts.onRename });

    // Session Info applies only to terminal drafts; browser/document drafts are not sessions.
    if (variant === "terminal") {
      items.push({
        label: t("tree.sessionInfo"),
        // Option/Alt opens advanced mode with the complete launch command.
        onClick: (e) => setDialog({ type: "sessionInfo", id: node.id, advanced: e.altKey }),
      });
    }

    if (!opts?.omitClose) {
      // Close Draft replaces End/Archive/Delete: terminals terminate and discard, browsers destroy their child WebView,
      // and documents use dirty-close confirmation.
      const closeDefault =
        variant === "doc"
          ? () => requestCloseDocTab(node.id)
          : variant === "browser"
            ? () => closeTab(node.id)
            : () => {
                void ptyKill(node.id).catch(() => {});
                closeSession(node.id);
              };
      items.push(sep, {
        label: t("tree.closeScratch"),
        danger: true,
        onClick: opts?.onClose ?? closeDefault,
      });
    }
    return items;
  };

  const handleSubmit = (values: Record<string, string>) => {
    if (!dialog) return;
    if (dialog.type === "editSession") {
      void updateSession(dialog.id, {
        name: values.name.trim(),
        shell: values.shell.trim() || null,
        cwd: values.cwd.trim() || null,
        initCmd: values.initCmd.trim() || null,
        // agentArgs appears only for agent sessions; undefined non-agent values become null.
        agentArgs: values.agentArgs?.trim() || null,
        // permissionMode appears only for supported agents; unchecked/hidden maps to null.
        permissionMode: values.permissionMode || null,
      });
    }
    setDialog(null);
  };

  const dialogs = (
    <>
      {dialog?.type === "newGroup" && (
        <NewGroup
          projectId={dialog.projectId}
          parentGroupId={dialog.parentGroupId}
          onCancel={() => setDialog(null)}
          onConfirm={async ({ name, worktree }) => {
            // Group worktree modes create normally, create an isolated worktree first, or bind an existing one.
            // Dialog remains open to show errors and closes only on success.
            if (worktree.mode === "new") {
              const st = useTermStore.getState();
              const parentGroup = dialog.parentGroupId
                ? st.groups.find((g) => g.id === dialog.parentGroupId)
                : null;
              const project = st.projects.find((p) => p.id === dialog.projectId);
              // Use the parent group's worktree as repository root when nested, otherwise project root.
              const repoRoot = parentGroup?.worktreePath || project?.rootPath || null;
              if (!repoRoot) throw new Error(t("worktree.noRepoRoot"));
              const wt = await createWorktree(repoRoot, worktree.name);
              await addGroup(dialog.projectId, dialog.parentGroupId, name, {
                worktreePath: wt.path,
                worktreeBaseRef: wt.baseRef || null,
              });
            } else if (worktree.mode === "existing") {
              await addGroup(dialog.projectId, dialog.parentGroupId, name, {
                worktreePath: worktree.path,
                worktreeBaseRef: null,
              });
            } else {
              await addGroup(dialog.projectId, dialog.parentGroupId, name);
            }
            setDialog(null);
          }}
        />
      )}
      {dialog?.type === "resumeSession" && (
        <ResumeSession
          onCancel={() => setDialog(null)}
          onConfirm={({ kind, sessionId, name, agentArgs, permissionMode }) => {
            void handleResumeSession(
              dialog.projectId,
              dialog.groupId,
              dialog.parentSessionId,
              kind,
              sessionId,
              name,
              { agentArgs, permissionMode },
            );
            setDialog(null);
          }}
        />
      )}
      {dialog?.type === "editSession" && (
        <FormModal
          title={t("tree.editSession")}
          fields={(() => {
            const k = sessions.find((s) => s.id === dialog.id)?.kind ?? "terminal";
            // WSL crossing is supported only for plain terminals; hide WSL from agent forms because hooks/executable
            // paths are incompatible across the boundary.
            const editableShells =
              k === "terminal" ? shells : shells.filter((shell) => !shell.id.startsWith("wsl:"));
            return sessionFields(
              editableShells,
              supportsAgentArgs(k),
              supportsPermissionToggle(k),
            );
          })()}
          initial={dialog.initial}
          submitLabel={t("common.save")}
          onSubmit={handleSubmit}
          onCancel={() => setDialog(null)}
        />
      )}
      {dialog?.type === "newAgentSession" && (
        <NewAgentSession
          projectId={dialog.projectId}
          parentSessionId={dialog.parentSessionId}
          initialWtMode={dialog.initialWtMode}
          onCancel={() => setDialog(null)}
          onConfirm={async ({ kind, name, agentArgs, permissionMode, worktree }) => {
            // Session worktree modes create normally, create an isolated worktree first, or bind an existing one.
            // Keep the dialog open on errors and close only on success.
            if (worktree.mode === "new") {
              await handleNewWorktreeSession(
                dialog.projectId,
                dialog.groupId,
                dialog.parentSessionId,
                { kind, name, worktreeName: worktree.name, agentArgs, permissionMode },
              );
            } else if (worktree.mode === "existing") {
              await handleNewSessionInWorktree(
                dialog.projectId,
                dialog.groupId,
                dialog.parentSessionId,
                { kind, name, worktreePath: worktree.path, agentArgs, permissionMode },
              );
            } else {
              await handleNewSession(
                dialog.projectId,
                dialog.groupId,
                kind,
                dialog.parentSessionId,
                null,
                { name, agentArgs, permissionMode },
              );
            }
            setDialog(null);
          }}
        />
      )}
      {dialog?.type === "sessionInfo" && (
        <SessionInfo id={dialog.id} advanced={dialog.advanced} onClose={() => setDialog(null)} />
      )}
      {dialog?.type === "groupInfo" && (
        <GroupInfo id={dialog.id} onClose={() => setDialog(null)} />
      )}
      {dialog?.type === "archiveBlocked" && (
        <ArchiveBlocked message={dialog.message} onClose={() => setDialog(null)} />
      )}
      {dialog?.type === "confirmArchive" && (
        <ConfirmArchive
          worktreePaths={dialog.worktreePaths}
          onCancel={() => setDialog(null)}
          onConfirm={() => {
            const { target, ids } = dialog;
            setDialog(null);
            const done =
              target === "group"
                ? archiveGroup(ids[0])
                : ids.length > 1
                  ? archiveMany(ids)
                  : archiveSession(ids[0]);
            void done.catch((e: unknown) =>
              setDialog({ type: "archiveBlocked", message: archiveErrorText(e) }),
            );
          }}
        />
      )}
      {dialog?.type === "confirmDelete" && (
        <ConfirmDelete
          name={dialog.node.name}
          kind={dialog.node.kind}
          worktreePaths={dialog.worktreePaths}
          onCancel={() => setDialog(null)}
          onConfirm={(removeWt) => {
            const { kind, id } = dialog.node;
            const paths = dialog.worktreePaths;
            void deleteNode(kind, id).then(() => {
              if (removeWt) {
                for (const p of paths) void removeWorktree(p, false).catch(() => {});
              }
            });
            setDialog(null);
          }}
        />
      )}
      {dialog?.type === "confirmDeleteMany" && (
        <ConfirmDelete
          name=""
          kind="session"
          batchCount={dialog.nodes.length}
          onCancel={() => setDialog(null)}
          onConfirm={() => {
            void deleteMany(dialog.nodes);
            setDialog(null);
          }}
        />
      )}
      {dialog?.type === "deleteWorktree" && (
        <DeleteWorktree
          repoRoot={dialog.repoRoot}
          defaultPath={dialog.defaultPath}
          onCancel={() => setDialog(null)}
          onConfirm={async ({ path, force }) => {
            // Delete only the worktree directory, not session/group bindings; conversion is a separate operation.
            // Keep the dialog open on errors and close on success.
            await removeWorktree(path, force);
            // Invalidate and immediately re-probe branch cache so the next context menu sees the missing directory and
            // exposes Convert to Normal without requiring two openings.
            invalidateGitBranch(path);
            prefetchGitBranchInfo(path);
            setDialog(null);
          }}
        />
      )}
    </>
  );

  return {
    newSessionItems,
    buildSessionItems,
    buildScratchItems,
    shellSwitchItems,
    buildMoveToMany,
    buildGitItems,
    buildMarkItem,
    openDialog: setDialog,
    dialogs,
  };
}

// Show custom launch arguments only for agents. Show the two-state skip-permissions toggle only for supported agents;
// OpenCode has no flag and Pi has no permission mechanism. Nonempty shells render a selector plus custom input;
// otherwise use a plain text field.
const sessionFields = (
  shells: ShellOption[],
  isAgent: boolean,
  canPermission: boolean,
): FieldDef[] => [
  { key: "name", label: t("tree.sessionName"), required: true, autoFocus: true },
  {
    key: "shell",
    label: t("tree.shellLabel"),
    placeholder: "/bin/zsh",
    // Detected shells render a dropdown plus custom entry; no detection retains the legacy plain text input.
    select: shells.length
      ? [
          { value: "", label: t("tree.shellSystemDefault") },
          ...shells.map((s) => ({ value: s.path, label: s.label })),
        ]
      : undefined,
    allowCustom: shells.length ? true : undefined,
  },
  { key: "cwd", label: t("tree.cwdLabel"), placeholder: "/path/to/dir" },
  { key: "initCmd", label: t("tree.initCmdLabel"), placeholder: "pnpm dev" },
  ...(isAgent
    ? [
        {
          key: "agentArgs",
          label: t("tree.agentArgsLabel"),
          placeholder: "--model opus",
          normalizeDashes: true,
        },
      ]
    : []),
  ...(canPermission
    ? [
        {
          key: "permissionMode",
          label: t("tree.permissionSkipLabel"),
          type: "checkbox" as const,
          checkedValue: "skip",
          hint: t("tree.permissionSkipHint"),
        },
      ]
    : []),
];
