//! State for the right panel's Git tab: repository status, the split change list, the commit count,
//! and the mutations the panel can run.
//!
//! Git has no change notification of its own, so everything here is pull-based: the tab polls while
//! it is on screen, refreshes when the window regains focus, and refreshes again the moment one of
//! its own mutations lands, so an action never leaves a stale row on screen for a poll interval.

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  gitChangedFiles,
  gitCommitCount,
  type ChangedFile,
} from "../../../ipc/commands";
import { getGitStatus } from "../../../ipc/info";
import type { GitStatus } from "../../../types";

/** Poll interval while the tab is visible. Git exposes no change events, so this is the refresh floor. */
const POLL_MS = 5000;

/** The three working-tree groups the panel lists, derived from each file's index and worktree sides. */
export interface ChangeGroups {
  /** Files with something staged. */
  staged: ChangedFile[];
  /** Tracked files changed in the worktree but not staged. */
  changes: ChangedFile[];
  /** Files git does not track yet. */
  untracked: ChangedFile[];
}

export interface GitPanelState {
  status: GitStatus | null;
  files: ChangedFile[] | null;
  groups: ChangeGroups;
  commitCount: number;
  isRepo: boolean;
  /** Load error for the whole tab, such as a directory that is not a repository. */
  err: string;
  /** Error from the last mutation the user triggered; cleared by the next successful one. */
  actionErr: string;
  /** True while a mutation is in flight, so the panel can disable its buttons. */
  busy: boolean;
  refresh: () => void;
  /** Counter bumped by every refresh, so panels holding their own paged data know to reload it. */
  tick: number;
  clearActionError: () => void;
  /** Runs one mutation, then refreshes. Resolves to true when it succeeded. */
  run: (action: () => Promise<unknown>) => Promise<boolean>;
}

/**
 * Split the flat backend list into the panel's groups. A partly staged file legitimately appears in
 * both `staged` and `changes`: that is what git's two status columns mean, and hiding one half would
 * misreport what a commit is about to include.
 */
export function groupChanges(files: ChangedFile[] | null): ChangeGroups {
  const groups: ChangeGroups = { staged: [], changes: [], untracked: [] };
  for (const f of files ?? []) {
    if (f.index) groups.staged.push(f);
    if (f.worktree === "untracked") groups.untracked.push(f);
    else if (f.worktree) groups.changes.push(f);
  }
  return groups;
}

export function useGitPanel(path: string | null): GitPanelState {
  const [status, setStatus] = useState<GitStatus | null>(null);
  const [files, setFiles] = useState<ChangedFile[] | null>(null);
  const [commitCount, setCommitCount] = useState(0);
  const [err, setErr] = useState("");
  const [actionErr, setActionErr] = useState("");
  const [busy, setBusy] = useState(false);
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((n) => n + 1), []);
  const clearActionError = useCallback(() => setActionErr(""), []);

  // Switching working directory clears the old repository's data; a poll tick refills it.
  useEffect(() => {
    setStatus(null);
    setFiles(null);
    setCommitCount(0);
    setErr("");
    setActionErr("");
  }, [path]);

  useEffect(() => {
    if (!path) return;
    let cancelled = false;
    void Promise.allSettled([
      getGitStatus(path),
      gitChangedFiles(path),
      gitCommitCount(path),
    ]).then(([s, f, c]) => {
      if (cancelled) return;
      const st = s.status === "fulfilled" ? s.value : null;
      setStatus(st);
      setCommitCount(c.status === "fulfilled" ? c.value : 0);
      if (f.status === "fulfilled") {
        setFiles(f.value);
        setErr("");
      } else {
        setFiles(null);
        // Outside a repository the changed-files call always fails; the branch row already says so.
        setErr(st && !st.isRepo ? "" : String(f.reason));
      }
    });
    return () => {
      cancelled = true;
    };
  }, [path, tick]);

  useEffect(() => {
    if (!path) return;
    const refreshIfVisible = () => {
      if (!document.hidden) refresh();
    };
    const id = setInterval(refreshIfVisible, POLL_MS);
    window.addEventListener("focus", refreshIfVisible);
    document.addEventListener("visibilitychange", refreshIfVisible);
    return () => {
      clearInterval(id);
      window.removeEventListener("focus", refreshIfVisible);
      document.removeEventListener("visibilitychange", refreshIfVisible);
    };
  }, [path, refresh]);

  const run = useCallback(
    async (action: () => Promise<unknown>) => {
      setBusy(true);
      try {
        await action();
        setActionErr("");
        return true;
      } catch (e) {
        setActionErr(String(e));
        return false;
      } finally {
        setBusy(false);
        refresh();
      }
    },
    [refresh],
  );

  const groups = useMemo(() => groupChanges(files), [files]);

  return {
    status,
    files,
    groups,
    commitCount,
    isRepo: status?.isRepo ?? false,
    err,
    actionErr,
    busy,
    refresh,
    tick,
    clearActionError,
    run,
  };
}
