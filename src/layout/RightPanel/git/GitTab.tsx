//! Right-panel Git tab: branch state on top, then the working tree split into Staged, Changes, and
//! Untracked files, a commit box, and the commit history.
//!
//! The split comes straight from git's two status columns (see `ChangedFile.index` / `.worktree`),
//! which is why a partly staged file shows up in two groups at once — that is what git means, and
//! collapsing it would misreport what the next commit will contain.
//!
//! Every action here goes through `run`, which refreshes the tab afterwards, so the list never sits
//! stale waiting for the next poll. Discarding asks first, inline on the row itself: it is the one
//! action that destroys work, and it only touches unstaged content, never what is already staged.

import { useState } from "react";
import Icons from "../../../components/Icons";
import { useT } from "../../../i18n";
import {
  gitDiscard,
  gitCommit,
  gitStage,
  gitUnstage,
  type ChangedFile,
} from "../../../ipc/commands";
import { useTermStore } from "../../../store/termStore";
import { CommitBox } from "./CommitBox";
import { CommitHistory } from "./CommitHistory";
import { GitFileRow, type RowAction } from "./GitFileRow";
import { GitSection } from "./GitSection";
import { useGitPanel } from "./useGitPanel";

/** Which groups start open. Committed history stays closed so a big repository costs nothing. */
const DEFAULT_OPEN = { staged: true, changes: true, untracked: true, commits: false };

type SectionKey = keyof typeof DEFAULT_OPEN;

export function GitTab({ path }: { path: string | null }) {
  const t = useT();
  const panel = useGitPanel(path);
  const openChanges = useTermStore((s) => s.openChanges);
  const [open, setOpen] = useState(DEFAULT_OPEN);
  // Path awaiting discard confirmation; only ever one row at a time.
  const [confirmDiscard, setConfirmDiscard] = useState<string | null>(null);

  const toggle = (key: SectionKey) => setOpen((o) => ({ ...o, [key]: !o[key] }));
  const cwd = path ?? "";
  const { groups, busy, isRepo } = panel;

  const paths = (files: ChangedFile[]) => files.map((f) => f.path);
  const stage = (files: ChangedFile[]) => void panel.run(() => gitStage(cwd, paths(files)));
  const unstage = (files: ChangedFile[]) => void panel.run(() => gitUnstage(cwd, paths(files)));
  const discard = (file: ChangedFile) => {
    setConfirmDiscard(null);
    void panel.run(() => gitDiscard(cwd, [file.path]));
  };

  /** Actions for a row in Staged: put it back, or open its diff. */
  const stagedActions = (f: ChangedFile): RowAction[] => [
    {
      key: "unstage",
      title: t("git.unstage"),
      icon: <Icons.minus size={13} />,
      onClick: () => unstage([f]),
    },
  ];

  /** Actions for a row in Changes or Untracked files: stage it, or throw the change away. */
  const changeActions = (f: ChangedFile): RowAction[] => [
    {
      key: "discard",
      title: f.worktree === "untracked" ? t("git.deleteFile") : t("git.discard"),
      icon: <Icons.restart size={13} />,
      onClick: () => setConfirmDiscard(f.path),
      danger: true,
    },
    {
      key: "stage",
      title: t("git.stage"),
      icon: <Icons.plus size={13} />,
      onClick: () => stage([f]),
    },
  ];

  const renderRows = (
    files: ChangedFile[],
    kind: "staged" | "changes" | "untracked",
  ) =>
    files.map((f) => {
      const staged = kind === "staged";
      return (
        <GitFileRow
          key={kind + ":" + f.path}
          file={f}
          additions={staged ? f.stagedAdditions : f.unstagedAdditions}
          deletions={staged ? f.stagedDeletions : f.unstagedDeletions}
          binary={staged ? f.stagedBinary : f.unstagedBinary}
          // Git reports a wholly untracked directory as one entry ending in a slash. There is no
          // file to diff behind it, so the row stages and discards but does not open a viewer.
          onOpen={f.path.endsWith("/") ? undefined : () => openChanges(cwd, { path: f.path })}
          actions={staged ? stagedActions(f) : changeActions(f)}
          confirming={!staged && confirmDiscard === f.path}
          confirmLabel={f.worktree === "untracked" ? t("git.deleteFile") : t("git.discard")}
          cancelLabel={t("common.cancel")}
          onConfirm={() => discard(f)}
          onCancelConfirm={() => setConfirmDiscard(null)}
        />
      );
    });

  return (
    <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
      <div className="git-head">
        <span className="ic">
          <Icons.branch size={13} />
        </span>
        <span className="br" title={panel.status?.branch ?? ""}>
          {isRepo ? (panel.status?.branch ?? t("git.detached")) : "—"}
        </span>
        {isRepo && (panel.status?.ahead || panel.status?.behind) ? (
          <span className="sync" title={t("git.aheadBehind")}>
            ↑{panel.status?.ahead} ↓{panel.status?.behind}
          </span>
        ) : null}
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
          onClick={panel.refresh}
        >
          ⟳
        </button>
      </div>

      {!isRepo && !panel.err ? (
        <div className="insp-empty git-pad">{t("changes.notRepo")}</div>
      ) : panel.err ? (
        <div className="insp-empty git-pad git-err">{panel.err}</div>
      ) : !panel.files ? (
        <div className="insp-empty git-pad">{t("changes.loading")}</div>
      ) : (
        <>
          {panel.actionErr && (
            <div className="git-action-err">
              <span>{panel.actionErr}</span>
              <button
                type="button"
                onClick={panel.clearActionError}
                title={t("common.close")}
                aria-label={t("common.close")}
              >
                <Icons.x size={11} />
              </button>
            </div>
          )}

          {panel.files.length === 0 && (
            <div className="insp-empty git-pad">{t("changes.noChanges")}</div>
          )}

          {panel.files.length > 0 && (
            <>
          <GitSection
            title={t("git.staged")}
            count={groups.staged.length}
            open={open.staged}
            onToggle={() => toggle("staged")}
            action={{
              label: t("git.unstageAll"),
              onClick: () => unstage(groups.staged),
              disabled: busy,
            }}
          >
            {renderRows(groups.staged, "staged")}
          </GitSection>

          <GitSection
            title={t("git.changes")}
            count={groups.changes.length}
            open={open.changes}
            onToggle={() => toggle("changes")}
            action={{
              label: t("git.stageAll"),
              onClick: () => stage(groups.changes),
              disabled: busy,
            }}
          >
            {renderRows(groups.changes, "changes")}
          </GitSection>

          <GitSection
            title={t("git.untracked")}
            count={groups.untracked.length}
            open={open.untracked}
            onToggle={() => toggle("untracked")}
            action={{
              label: t("git.stageAll"),
              onClick: () => stage(groups.untracked),
              disabled: busy,
            }}
          >
            {renderRows(groups.untracked, "untracked")}
          </GitSection>
            </>
          )}

          <CommitBox
            stagedCount={groups.staged.length}
            busy={busy}
            onCommit={(message, amend) => panel.run(() => gitCommit(cwd, message, amend))}
          />

          <GitSection
            title={t("git.committed")}
            count={panel.commitCount}
            open={open.commits}
            onToggle={() => toggle("commits")}
          >
            {open.commits && <CommitHistory cwd={cwd} total={panel.commitCount} tick={panel.tick} />}
          </GitSection>
        </>
      )}
    </div>
  );
}
