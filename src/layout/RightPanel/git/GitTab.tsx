//! Right-panel Git tab: branch and remote sync on top, then the working tree split into Conflicts,
//! Staged, Changes, and Untracked files, a commit box, and the commit history.
//!
//! The split comes straight from git's two status columns (see `ChangedFile.index` / `.worktree`),
//! which is why a partly staged file shows up in two groups at once — that is what git means, and
//! collapsing it would misreport what the next commit will contain.
//!
//! Every action here goes through `run`, which refreshes the tab afterwards, so the list never sits
//! stale waiting for the next poll. Discarding asks first — inline on the row for one file, at the
//! top of the group for several — because it is the one action that destroys work; it only touches
//! unstaged content, never what is already staged.
//!
//! Rows select like a file list: a click selects one file and opens its diff, Cmd/Ctrl-click
//! toggles, Shift-click extends a range. A selection lives inside one group, since what can be done
//! to a file depends on the group it is in.

import { useEffect, useMemo, useState } from "react";
import { ContextMenu, type MenuItem } from "../../../components/ContextMenu";
import Icons from "../../../components/Icons";
import Select from "../../../components/Select";
import { useT } from "../../../i18n";
import {
  gitAbortOperation,
  gitContinueOperation,
  gitDiscard,
  gitCommit,
  gitFetch,
  gitPull,
  gitPush,
  gitStage,
  gitUnstage,
  type ChangedFile,
  type CommitInfo,
  type GitChangeSide,
} from "../../../ipc/commands";
import { copyText, revealPath } from "../../../ipc/info";
import { useTermStore } from "../../../store/termStore";
import type { Session } from "../../../types";
import { useAgentHandoff } from "./agentHandoff";
import { BranchCompareSection } from "./BranchCompareSection";
import { CommitBox } from "./CommitBox";
import { CommitHistory } from "./CommitHistory";
import { GitDirRow, GitFileRow, type RowAction } from "./GitFileRow";
import { GitOperationBanner } from "./GitOperationBanner";
import { GitConfirm, GitSection, type SectionAction } from "./GitSection";
import { GitSyncBar, type RemoteAction } from "./GitSyncBar";
import { buildGitRows, type GitViewMode } from "./gitTree";
import { useGitPanel } from "./useGitPanel";
import { filesForSide } from "../../../components/diff/changeGroups";
import { useRepoTarget } from "./useRepoTarget";

/** Which groups start open. Committed history stays closed so a big repository costs nothing. */
const DEFAULT_OPEN = { staged: true, changes: true, untracked: true, conflicts: true, branch: false, commits: false };

type SectionKey = keyof typeof DEFAULT_OPEN;
/** A file group; each maps to one comparison side. */
type Kind = "conflicted" | "staged" | "changes" | "untracked";

const SECTION: Record<Kind, SectionKey> = { conflicted: "conflicts", staged: "staged", changes: "changes", untracked: "untracked" };
const sideOf = (kind: Kind): GitChangeSide => (kind === "changes" ? "unstaged" : kind);

const VIEW_KEY = "vlx.git.viewMode";
function readViewMode(): GitViewMode {
  try {
    return localStorage.getItem(VIEW_KEY) === "tree" ? "tree" : "list";
  } catch {
    return "list";
  }
}

/** Selected files within one group, plus the row a Shift-click range starts from. */
interface Selection {
  kind: Kind;
  paths: string[];
  anchor: string;
}

/** A discard waiting for confirmation: one row inline, or several files at the top of their group. */
interface PendingDiscard {
  kind: Kind;
  paths: string[];
  /** Set for a single-row confirmation, which renders on that row. */
  row?: string;
}

/** Paths from `from` to `to` in display order, inclusive, whichever comes first. */
export function rangeBetween(order: string[], from: string, to: string): string[] {
  const a = order.indexOf(from);
  const b = order.indexOf(to);
  if (a < 0 || b < 0) return [to];
  return order.slice(Math.min(a, b), Math.max(a, b) + 1);
}

export function GitTab({ path, session }: { path: string | null; session?: Session }) {
  const t = useT();
  // On a folder holding several repositories, the tab works on the one picked above the branch row.
  const target = useRepoTarget(path);
  const panel = useGitPanel(target.repoPath);
  const openChanges = useTermStore((s) => s.openChanges);
  const agent = useAgentHandoff(session);
  const [open, setOpen] = useState(DEFAULT_OPEN);
  const [filter, setFilter] = useState("");
  const [viewMode, setViewMode] = useState<GitViewMode>(readViewMode);
  // Folders closed in tree view, keyed by group and folder path.
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());
  const [selection, setSelection] = useState<Selection | null>(null);
  const [pending, setPending] = useState<PendingDiscard | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);
  // Remote action in flight, so its button can say what is happening while the network works.
  const [remoteOp, setRemoteOp] = useState<RemoteAction | null>(null);

  const toggle = (key: SectionKey) => setOpen((o) => ({ ...o, [key]: !o[key] }));
  const cwd = target.repoPath ?? "";
  const { groups, busy, isRepo } = panel;
  const status = panel.status;
  const operation = status?.operation ?? null;
  const query = filter.trim().toLocaleLowerCase();
  const matches = (f: ChangedFile) => !query || `${f.path} ${f.originalPath ?? ""}`.toLocaleLowerCase().includes(query);

  // Each group's files as the comparison side sees them, filtered, in display order.
  const visible = useMemo(() => {
    const side = (files: ChangedFile[], kind: Kind) => filesForSide(files, sideOf(kind)).filter(matches);
    return {
      conflicted: side(groups.conflicts, "conflicted"),
      staged: side(groups.staged, "staged"),
      changes: side(groups.changes, "changes"),
      untracked: side(groups.untracked, "untracked"),
    };
    // `matches` only reads `query`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [groups, query]);

  const rowsOf = (kind: Kind) => buildGitRows(visible[kind], viewMode, new Set(
    [...collapsed].filter((key) => key.startsWith(kind + ":")).map((key) => key.slice(kind.length + 1)),
  ));

  // Drop selected and pending paths that left their group, e.g. after staging them.
  const selected = useMemo(() => {
    if (!selection) return null;
    const present = new Set(visible[selection.kind].map((f) => f.path));
    const paths = selection.paths.filter((p) => present.has(p));
    return paths.length ? { ...selection, paths } : null;
  }, [selection, visible]);

  // A different repository has different files.
  useEffect(() => {
    setSelection(null);
    setPending(null);
    setCollapsed(new Set());
  }, [cwd]);

  const setMode = (mode: GitViewMode) => {
    setViewMode(mode);
    try {
      localStorage.setItem(VIEW_KEY, mode);
    } catch {
      // The choice then lasts for this tab only.
    }
  };

  const stagePaths = (paths: string[]) => void panel.run(() => gitStage(cwd, paths));
  const unstagePaths = (paths: string[]) => void panel.run(() => gitUnstage(cwd, paths));
  const discardPaths = (paths: string[]) => {
    setPending(null);
    void panel.run(() => gitDiscard(cwd, paths));
  };
  const askDiscard = (kind: Kind, paths: string[], single: boolean) => {
    setPending({ kind, paths, row: single ? paths[0] : undefined });
    if (!single) setOpen((o) => ({ ...o, [SECTION[kind]]: true }));
  };

  const remote = (action: RemoteAction, call: () => Promise<unknown>) => {
    setRemoteOp(action);
    void panel.run(call).finally(() => setRemoteOp(null));
  };

  // A fast-forward-only pull refused because local and upstream both have new commits: offer the
  // two ways to combine them right there instead of leaving only git's message.
  const diverged = Boolean(status?.upstream) && /fast-forward/i.test(panel.actionErr);

  const root = status?.root || cwd;
  const absolute = (p: string) => `${root.replace(/[\\/]+$/, "")}/${p.replace(/\/$/, "")}`;
  const openable = (f: ChangedFile) => !f.path.endsWith("/");
  // A file deleted in the worktree has nothing to open in an editor.
  const onDisk = (f: ChangedFile) => f.worktree !== "deleted" && !(f.status === "deleted" && !f.worktree);

  // A failed chat delivery shows in the conversation's own outbox; a failed paste leaves the
  // terminal as it was, which the user sees when the session opens.
  const ask = (text: string) => {
    if (agent) void agent.send(text).catch(() => {});
  };
  const askResolve = (paths: string[]) => ask(t("git.ask.resolvePrompt", paths.map((p) => `- ${p}`).join("\n")));

  /** What each group can do to a set of its files: stage, unstage, discard, or mark resolved. */
  const groupActions = (kind: Kind, paths: string[], single: boolean): RowAction[] => {
    switch (kind) {
      case "staged":
        return [{ key: "unstage", title: t("git.unstage"), icon: <Icons.minus size={13} />, onClick: () => unstagePaths(paths) }];
      case "conflicted":
        return [{ key: "resolve", title: t("git.markResolved"), icon: <Icons.check size={13} />, onClick: () => stagePaths(paths) }];
      case "changes":
      case "untracked":
        return [
          {
            key: "discard",
            title: kind === "untracked" ? t("git.deleteFile") : t("git.discard"),
            icon: <Icons.restart size={13} />,
            onClick: () => askDiscard(kind, paths, single),
            danger: true,
          },
          { key: "stage", title: t("git.stage"), icon: <Icons.plus size={13} />, onClick: () => stagePaths(paths) },
        ];
    }
  };

  const toMenu = (actions: RowAction[]): MenuItem[] =>
    actions.map((a) => ({ label: a.title, icon: a.icon, onClick: a.onClick, danger: a.danger, disabled: busy }));

  const openMenu = (e: React.MouseEvent, items: MenuItem[]) => {
    e.preventDefault();
    setMenu({ x: e.clientX, y: e.clientY, items });
  };

  const fileMenu = (kind: Kind, f: ChangedFile): MenuItem[] => {
    // Right-clicking inside a multi-selection acts on all of it.
    const many = selected?.kind === kind && selected.paths.length > 1 && selected.paths.includes(f.path);
    const paths = many ? selected!.paths : [f.path];
    const items: MenuItem[] = [];
    if (!many) {
      items.push(
        { label: t("git.openChanges"), icon: <Icons.docLines size={14} />, disabled: !openable(f),
          onClick: () => openChanges(cwd, { path: f.path, side: sideOf(kind) }) },
        { label: t("panel.openInEditor"), icon: <Icons.code size={14} />, disabled: !openable(f) || !onDisk(f),
          onClick: () => useTermStore.getState().openDocTab(absolute(f.path)) },
        { label: "", separator: true },
      );
    }
    items.push(...toMenu(groupActions(kind, paths, !many)));
    if (kind === "conflicted" && agent) {
      items.push({ label: t("git.ask.resolve", agent.label), icon: <Icons.sparkle size={14} />, onClick: () => askResolve(paths) });
    }
    if (!many) {
      items.push(
        { label: "", separator: true },
        { label: t("files.copyPath"), icon: <Icons.copy size={14} />, onClick: () => void copyText(absolute(f.path)) },
        { label: t("files.copyRelPath"), icon: <Icons.copy size={14} />, onClick: () => void copyText(f.path.replace(/\/$/, "")) },
        { label: t("files.revealInFinder"), icon: <Icons.reveal size={14} />, disabled: !onDisk(f),
          onClick: () => void revealPath(absolute(f.path)) },
      );
    }
    return items;
  };

  const clickRow = (kind: Kind, f: ChangedFile, e: React.MouseEvent) => {
    const order = visible[kind].map((v) => v.path);
    const same = selected?.kind === kind ? selected : null;
    if (e.shiftKey && same) {
      setSelection({ kind, paths: rangeBetween(order, same.anchor, f.path), anchor: same.anchor });
      return;
    }
    if (e.metaKey || e.ctrlKey) {
      const paths = same?.paths.includes(f.path) ? same.paths.filter((p) => p !== f.path) : [...(same?.paths ?? []), f.path];
      setSelection(paths.length ? { kind, paths, anchor: f.path } : null);
      return;
    }
    setSelection({ kind, paths: [f.path], anchor: f.path });
    if (openable(f)) openChanges(cwd, { path: f.path, side: sideOf(kind) });
  };

  const renderGroup = (kind: Kind) => {
    const groupPending = pending?.kind === kind && !pending.row ? pending : null;
    const bulk = selected?.kind === kind && selected.paths.length > 1 ? selected : null;
    const untracked = kind === "untracked";
    return (
      <>
        {groupPending && (
          <GitConfirm
            text={untracked ? t("git.deleteConfirm", groupPending.paths.length) : t("git.discardConfirm", groupPending.paths.length)}
            confirmLabel={untracked ? t("git.deleteFile") : t("git.discard")}
            cancelLabel={t("common.cancel")}
            disabled={busy}
            onConfirm={() => discardPaths(groupPending.paths)}
            onCancel={() => setPending(null)}
          />
        )}
        {bulk && !groupPending && (
          <div className="git-bulk" role="toolbar" aria-label={t("git.selectedCount", bulk.paths.length)}>
            <span className="git-bulk-count">{t("git.selectedCount", bulk.paths.length)}</span>
            {groupActions(kind, bulk.paths, false).map((a) => (
              <button key={a.key} type="button" className={"vlx-btn" + (a.danger ? " git-danger-btn" : "")} disabled={busy} onClick={a.onClick}>
                {a.title}
              </button>
            ))}
            <button type="button" className="git-bulk-clear" title={t("git.clearSelection")} aria-label={t("git.clearSelection")}
              onClick={() => setSelection(null)}>
              <Icons.x size={11} />
            </button>
          </div>
        )}
        {rowsOf(kind).map((row) => {
          if (row.kind === "dir") {
            const key = `${kind}:${row.path}`;
            const paths = row.files.map((f) => f.path);
            const actions = groupActions(kind, paths, false);
            return (
              <GitDirRow
                key={"d:" + key}
                name={row.name}
                path={row.path}
                depth={row.depth}
                open={row.open}
                count={row.files.length}
                actions={actions}
                disabled={busy}
                onToggle={() => setCollapsed((c) => {
                  const next = new Set(c);
                  if (next.has(key)) next.delete(key);
                  else next.add(key);
                  return next;
                })}
                onContextMenu={(e) => openMenu(e, [
                  ...toMenu(actions),
                  ...(kind === "conflicted" && agent
                    ? [{ label: t("git.ask.resolve", agent.label), icon: <Icons.sparkle size={14} />, onClick: () => askResolve(paths) }]
                    : []),
                  { label: "", separator: true },
                  { label: t("files.copyPath"), icon: <Icons.copy size={14} />, onClick: () => void copyText(absolute(row.path)) },
                  { label: t("files.copyRelPath"), icon: <Icons.copy size={14} />, onClick: () => void copyText(row.path) },
                ])}
              />
            );
          }
          const f = row.file;
          const isPending = pending?.kind === kind && pending.row === f.path;
          return (
            <GitFileRow
              key={kind + ":" + f.path}
              file={f}
              additions={f.additions}
              deletions={f.deletions}
              binary={f.binary}
              disabled={busy}
              selected={Boolean(selected?.kind === kind && selected.paths.includes(f.path))}
              depth={row.depth}
              tree={viewMode === "tree"}
              onOpen={(e) => clickRow(kind, f, e)}
              onContextMenu={(e) => openMenu(e, fileMenu(kind, f))}
              actions={groupActions(kind, [f.path], true)}
              confirming={isPending}
              confirmLabel={f.worktree === "untracked" ? t("git.deleteFile") : t("git.discard")}
              cancelLabel={t("common.cancel")}
              onConfirm={() => discardPaths([f.path])}
              onCancelConfirm={() => setPending(null)}
            />
          );
        })}
      </>
    );
  };

  const commitMenu = (commit: CommitInfo, e: React.MouseEvent) => openMenu(e, [
    { label: t("git.copyCommitHash"), icon: <Icons.copy size={14} />, onClick: () => void copyText(commit.oid ?? commit.hash) },
    { label: t("git.copyCommitMessage"), icon: <Icons.copy size={14} />, onClick: () => void copyText(commit.message ?? commit.subject) },
    ...(agent
      ? [{ label: t("git.ask.explain", agent.label), icon: <Icons.sparkle size={14} />,
          onClick: () => ask(t("git.ask.explainPrompt", commit.oid ?? commit.hash, commit.subject)) }]
      : []),
  ]);

  // Group-wide actions act on the whole group, so they wait while a filter hides part of it.
  const filtered = busy || Boolean(query);
  const sectionActions: Record<Kind, SectionAction[]> = {
    conflicted: [],
    staged: [{ key: "unstage", label: t("git.unstageAll"), icon: <Icons.minus size={13} />,
      onClick: () => unstagePaths(visible.staged.map((f) => f.path)), disabled: filtered }],
    changes: [
      { key: "discard", label: t("git.discardAll"), icon: <Icons.restart size={13} />, danger: true,
        onClick: () => askDiscard("changes", visible.changes.map((f) => f.path), false), disabled: filtered },
      { key: "stage", label: t("git.stageAll"), icon: <Icons.plus size={13} />,
        onClick: () => stagePaths(visible.changes.map((f) => f.path)), disabled: filtered },
    ],
    untracked: [
      { key: "delete", label: t("git.deleteAll"), icon: <Icons.trash size={13} />, danger: true,
        onClick: () => askDiscard("untracked", visible.untracked.map((f) => f.path), false), disabled: filtered },
      { key: "stage", label: t("git.stageAll"), icon: <Icons.plus size={13} />,
        onClick: () => stagePaths(visible.untracked.map((f) => f.path)), disabled: filtered },
    ],
  };

  return (
    <div
      className="git-panel"
      data-git-scroll
      style={{ flex: 1, minHeight: 0, overflowY: "auto" }}
      onKeyDown={(e) => {
        if (e.key === "Escape" && (selected || pending)) {
          setSelection(null);
          setPending(null);
        }
      }}
    >
      {target.repos.length > 0 && target.repoPath && (
        <div className="git-repo">
          <Select
            value={target.repoPath}
            onChange={target.select}
            options={target.repos.map((r) => ({ value: r.path, label: r.name }))}
            width="100%"
            size="sm"
            leading={t("git.repository")}
            title={target.repoPath}
            menuPortal
          />
        </div>
      )}
      <div className="git-head">
        <span className="ic">
          <Icons.branch size={13} />
        </span>
        <span className="br" title={status?.branch ?? ""}>
          {isRepo ? (status?.branch ?? t("git.detached")) : "—"}
        </span>
        {isRepo && status?.upstream && (
          <span className="up" title={t("git.upstreamTitle", status.upstream)}>
            {status.upstream}
          </span>
        )}
        {isRepo && (
          <button
            type="button"
            className="git-sec-act git-head-act"
            onClick={() => openChanges(cwd)}
            title={t("git.viewAll")}
          >
            {t("git.viewAll")}
          </button>
        )}
        <button
          type="button"
          className="insp-refresh"
          title={t("changes.refresh")}
          aria-label={t("changes.refresh")}
          onClick={() => {
            target.rescan();
            panel.refresh();
          }}
        >
          ⟳
        </button>
      </div>

      {isRepo && status && (
        <GitSyncBar
          status={status}
          running={remoteOp}
          disabled={busy}
          onFetch={() => remote("fetch", () => gitFetch(cwd))}
          onPull={(mode) => remote("pull", () => gitPull(cwd, mode))}
          onPush={(force) => remote(status.upstream ? "push" : "publish", () => gitPush(cwd, force))}
        />
      )}

      {isRepo && panel.files && panel.files.length > 0 && <div className="git-filter">
        <div className="git-filter-head">
          <label htmlFor="git-file-filter">
            {panel.truncated ? t("git.changedFilesOver", panel.files.length) : t("git.changedFiles", panel.files.length)}
          </label>
          <button
            type="button"
            className="git-view-toggle"
            title={viewMode === "tree" ? t("git.viewAsList") : t("git.viewAsTree")}
            aria-label={viewMode === "tree" ? t("git.viewAsList") : t("git.viewAsTree")}
            onClick={() => setMode(viewMode === "tree" ? "list" : "tree")}
          >
            {viewMode === "tree" ? <Icons.menu size={13} /> : <Icons.folder size={13} />}
          </button>
        </div>
        <input id="git-file-filter" className="vlx-input" type="search" value={filter}
          onChange={(e) => setFilter(e.target.value)} placeholder={t("git.filterFiles")} />
        {query && !panel.files.some(matches) && <div className="insp-empty">{t("git.noMatchingFiles")}</div>}
      </div>}

      {path && !target.repoPath ? (
        <div className="insp-empty git-pad">{t("changes.loading")}</div>
      ) : path && !panel.status && !panel.err ? (
        <div className="insp-empty git-pad">{t("changes.loading")}</div>
      ) : !isRepo && !panel.err ? (
        <div className="insp-empty git-pad">{t("changes.notRepo")}</div>
      ) : panel.err && !panel.files ? (
        <div className="insp-empty git-pad git-err">{panel.err}</div>
      ) : !panel.files ? (
        <div className="insp-empty git-pad">{t("changes.loading")}</div>
      ) : (
        <>
          {panel.truncated && (
            <div className="git-limit" role="status">
              <span>{t("git.tooManyChanges", panel.files.length)}</span>
              {panel.ignoreSuggestion && (
                <div className="git-limit-suggest">
                  <span className="git-limit-text">{t("git.ignoreSuggestion", panel.ignoreSuggestion)}</span>
                  <button type="button" className="vlx-btn" disabled={busy} onClick={panel.ignoreFolder}>
                    {t("git.addToGitignore")}
                  </button>
                  <button
                    type="button"
                    className="git-limit-close"
                    onClick={panel.dismissIgnoreSuggestion}
                    title={t("common.close")}
                    aria-label={t("common.close")}
                  >
                    <Icons.x size={11} />
                  </button>
                </div>
              )}
            </div>
          )}
          {(panel.actionErr || panel.err) && (
            <div className="git-action-err">
              <span className="git-action-err-text">
                {panel.actionErr || panel.err}
                {diverged && (
                  <span className="git-diverged">
                    <span>{t("git.divergedHint")}</span>
                    <span className="git-diverged-acts">
                      <button type="button" className="vlx-btn" disabled={busy} onClick={() => remote("pull", () => gitPull(cwd, "rebase"))}>
                        {t("git.pullRebase")}
                      </button>
                      <button type="button" className="vlx-btn" disabled={busy} onClick={() => remote("pull", () => gitPull(cwd, "merge"))}>
                        {t("git.pullMerge")}
                      </button>
                    </span>
                  </span>
                )}
              </span>
              {panel.actionErr && agent && (
                <button
                  type="button"
                  className="git-action-err-ask"
                  onClick={() => ask(t("git.ask.fixPrompt", panel.actionErr))}
                >
                  {t("git.ask.fix", agent.label)}
                </button>
              )}
              {panel.actionErr && (
              <button
                type="button"
                onClick={panel.clearActionError}
                title={t("common.close")}
                aria-label={t("common.close")}
              >
                <Icons.x size={11} />
              </button>
              )}
            </div>
          )}

          {operation && (
            <GitOperationBanner
              operation={operation}
              conflicts={groups.conflicts.length}
              disabled={busy}
              onContinue={() => void panel.run(() => gitContinueOperation(cwd))}
              onAbort={() => void panel.run(() => gitAbortOperation(cwd))}
              agentLabel={agent?.label ?? null}
              onAskAgent={() => askResolve(groups.conflicts.map((f) => f.path))}
            />
          )}

          {panel.files.length === 0 && (
            <div className="insp-empty git-pad">{t("changes.noChanges")}</div>
          )}

          {panel.files.length > 0 && (
            <>
          {groups.conflicts.length > 0 && <GitSection title={t("git.conflicts")} count={visible.conflicted.length}
            open={open.conflicts} onToggle={() => toggle("conflicts")}
            view={{ label: t("git.viewAll"), onClick: () => openChanges(cwd, { side: "conflicted" }) }}>
            <div className="git-conflict-note">{t("git.conflictNotice")}</div>
            {renderGroup("conflicted")}
          </GitSection>}
          <GitSection
            title={t("git.staged")}
            count={visible.staged.length}
            view={{ label: t("git.viewAll"), onClick: () => openChanges(cwd, { side: "staged" }) }}
            open={open.staged}
            onToggle={() => toggle("staged")}
            actions={sectionActions.staged}
          >
            {renderGroup("staged")}
          </GitSection>

          <GitSection
            title={t("git.changes")}
            count={visible.changes.length}
            view={{ label: t("git.viewAll"), onClick: () => openChanges(cwd, { side: "unstaged" }) }}
            open={open.changes}
            onToggle={() => toggle("changes")}
            actions={sectionActions.changes}
          >
            {renderGroup("changes")}
          </GitSection>

          <GitSection
            title={t("git.untracked")}
            count={visible.untracked.length}
            view={{ label: t("git.viewAll"), onClick: () => openChanges(cwd, { side: "untracked" }) }}
            open={open.untracked}
            onToggle={() => toggle("untracked")}
            actions={sectionActions.untracked}
          >
            {renderGroup("untracked")}
          </GitSection>
            </>
          )}

          {/* A stopped operation is finished with Continue, which keeps the message git prepared;
              a plain commit in the middle of a rebase would add an extra commit instead. */}
          {!operation && <CommitBox
            key={cwd}
            cwd={cwd}
            head={status?.head ?? null}
            stagedCount={groups.staged.length}
            busy={busy || groups.conflicts.length > 0}
            onCommit={(message, amend, expectHead) => panel.run(() => gitCommit(cwd, message, amend, expectHead))}
            agentLabel={agent?.label ?? null}
            onAskAgent={() => ask(t("git.ask.commitPrompt"))}
          />}

          <BranchCompareSection
            key={"compare:" + cwd}
            cwd={cwd}
            root={root}
            head={status?.head ?? null}
            branch={status?.branch ?? null}
            refreshToken={panel.historyRefresh}
            open={open.branch}
            onToggle={() => toggle("branch")}
            session={session}
          />

          <GitSection
            title={t("git.committed")}
            count={panel.commitCount}
            open={open.commits}
            onToggle={() => toggle("commits")}
          >
            {open.commits && <CommitHistory key={cwd} cwd={cwd} total={panel.commitCount} head={status?.head ?? null}
              refreshToken={panel.historyRefresh} onContextMenu={commitMenu} />}
          </GitSection>
        </>
      )}
      {menu && <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={() => setMenu(null)} />}
    </div>
  );
}
