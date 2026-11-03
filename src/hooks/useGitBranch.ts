//! Lightweight Git branch/worktree information for pane-header and status-bar branch chips.
//! Cache by path and deduplicate same-path requests so multiple panes do not probe one repository repeatedly.
//!
//! Live branch labels share a watch lease and refresh on HEAD/reference changes. Sidebar icons opt into
//! cached metadata only: rendering a closed session must not create a persistent watch or poll.
//! Window focus re-probes every live path, which covers platforms that cannot watch (Linux) and linked
//! worktrees, whose HEAD lives in the main repository's git directory outside the watched root.

import { useEffect, useMemo, useState } from "react";
import { gitWatch } from "../ipc/commands";
import { onFsChanged } from "../ipc/events";
import { getGitBranchInfo } from "../ipc/info";

/** Git branch and worktree information resolved for a directory. */
export interface GitBranchInfo {
  /** Whether the directory is inside a Git repository; false on probe failure. */
  isRepo: boolean;
  /** Branch name; null outside a repository or under detached HEAD. */
  branch: string | null;
  /** Whether this is a linked Git worktree rather than the main worktree. */
  isWorktree: boolean;
  /** Top-level worktree path, present only for isWorktree and used in directory-name tooltips. */
  worktreePath: string | null;
}

const EMPTY: GitBranchInfo = {
  isRepo: false,
  branch: null,
  isWorktree: false,
  worktreePath: null,
};

/** Renew watch leases (the backend drops them after 90 s) and poll unwatched repositories at this pace. */
const RENEW_MS = 30_000;
/** Merge the burst of `.git` writes one command produces into a single probe. */
const EVENT_DEBOUNCE_MS = 150;

const cache = new Map<string, GitBranchInfo>();
const inflight = new Map<string, Promise<GitBranchInfo>>();
/** At most one follow-up per path while a request is running. */
const pendingRefresh = new Set<string>();
/** Probe sequence per path; only the newest probe may write the cache, so a slow stale answer cannot win. */
const seq = new Map<string, number>();

type Listener = (info: GitBranchInfo) => void;

/** A path some mounted component displays: its listeners and the watched repository root, if any. */
interface Tracked {
  listeners: Set<Listener>;
  liveListeners: Set<Listener>;
  /** Root that `fs://changed` events carry for this path; null until the lease answers, or when unwatchable. */
  root: string | null;
}

const tracked = new Map<string, Tracked>();

function sameInfo(a: GitBranchInfo, b: GitBranchInfo): boolean {
  return a.isRepo === b.isRepo && a.branch === b.branch && a.isWorktree === b.isWorktree &&
    a.worktreePath === b.worktreePath;
}

/** Serialize probes; a fresh request during a running probe schedules one follow-up. */
function probe(path: string, fresh = false): Promise<GitBranchInfo> {
  let p = inflight.get(path);
  if (p && fresh) pendingRefresh.add(path);
  if (!p) {
    const n = (seq.get(path) ?? 0) + 1;
    seq.set(path, n);
    const req = getGitBranchInfo(path)
      .then((s) => ({
        isRepo: !!s.isRepo,
        branch: s.isRepo ? (s.branch ?? null) : null,
        isWorktree: !!s.isWorktree,
        worktreePath: s.worktreePath ?? null,
      }))
      .catch(() => EMPTY);
    p = req;
    inflight.set(path, req);
    void req.then((info) => {
      if (inflight.get(path) === req) inflight.delete(path);
      const repeat = pendingRefresh.delete(path) && !!tracked.get(path)?.liveListeners.size;
      if (repeat) { void probe(path); return; }
      if (seq.get(path) !== n) return;
      const prev = cache.get(path);
      cache.set(path, info);
      if (!prev || !sameInfo(prev, info)) {
        for (const listener of tracked.get(path)?.listeners ?? []) listener(info);
      }
    });
  }
  return p;
}

/** Ask for or renew the watch lease on the repository containing a tracked path. */
function renewWatch(path: string, entry: Tracked): void {
  void gitWatch(path)
    .then((info) => { if (tracked.get(path) === entry) entry.root = info.supported ? info.root : null; })
    .catch(() => { if (tracked.get(path) === entry) entry.root = null; });
}

/** Window-level wiring shared by every tracked path; installed with the first and removed with the last. */
let stopShared: (() => void) | null = null;

function startShared(): void {
  if (stopShared) return;
  let pending = new Set<string>();
  let debounce: ReturnType<typeof setTimeout> | null = null;
  const unlisten = onFsChanged(({ root, branch }) => {
    if (!branch || document.hidden) return;
    for (const [path, entry] of tracked) if (entry.liveListeners.size && entry.root === root) pending.add(path);
    if (debounce || !pending.size) return;
    debounce = setTimeout(() => {
      debounce = null;
      const paths = pending;
      pending = new Set();
      for (const path of paths) if (tracked.get(path)?.liveListeners.size) void probe(path, true);
    }, EVENT_DEBOUNCE_MS);
  });
  // Watched repositories only need their lease renewed. Unwatched ones, and linked worktrees whose HEAD
  // sits outside the watched root, are re-probed instead; non-repositories wait for focus.
  const timer = setInterval(() => {
    if (document.hidden) return;
    for (const [path, entry] of tracked) {
      if (!entry.liveListeners.size) continue;
      if (entry.root) renewWatch(path, entry);
      const info = cache.get(path);
      if (info?.isRepo && (!entry.root || info.isWorktree)) void probe(path, true);
    }
  }, RENEW_MS);
  const recheckAll = () => {
    if (document.hidden) return;
    for (const [path, entry] of tracked) if (entry.liveListeners.size) void probe(path, true);
  };
  window.addEventListener("focus", recheckAll);
  document.addEventListener("visibilitychange", recheckAll);
  stopShared = () => {
    if (debounce) clearTimeout(debounce);
    void unlisten.then((stop) => stop()).catch(() => {});
    clearInterval(timer);
    window.removeEventListener("focus", recheckAll);
    document.removeEventListener("visibilitychange", recheckAll);
  };
}

/** Keep a path's cache live while a listener displays it; returns the matching release. */
function track(path: string, listener: Listener, live: boolean): () => void {
  let entry = tracked.get(path);
  if (!entry) {
    entry = { listeners: new Set(), liveListeners: new Set(), root: null };
    tracked.set(path, entry);
  }
  entry.listeners.add(listener);
  if (live) {
    entry.liveListeners.add(listener);
    if (entry.liveListeners.size === 1) renewWatch(path, entry);
    startShared();
  }
  const own = entry;
  return () => {
    own.listeners.delete(listener);
    own.liveListeners.delete(listener);
    if (tracked.get(path) !== own) return;
    if (!own.liveListeners.size) own.root = null;
    if (!own.listeners.size) tracked.delete(path);
    if (![...tracked.values()].some((e) => e.liveListeners.size) && stopShared) {
      stopShared();
      stopShared = null;
    }
  };
}

/** Read cached branch information synchronously; return undefined without requesting if never probed. Used to build session context menus. */
export function peekGitBranchInfo(path?: string | null): GitBranchInfo | undefined {
  return path ? cache.get(path) : undefined;
}

/** Trigger a cache-filling probe unless cached/in flight; menus warm it on open so the next open has a result. */
export function prefetchGitBranchInfo(path?: string | null): void {
  if (!path || cache.has(path)) return;
  void probe(path);
}

/** Invalidate a path after deleting a worktree. The next peek returns undefined and triggers a probe, which sees
 *  the missing directory as isRepo:false and enables the Convert to Regular Session/Group menu. */
export function invalidateGitBranch(path?: string | null): void {
  if (path) cache.delete(path);
}

/** Whether a worktree path has been deleted, inferred from branch cache: probed and not a repository means the
 *  directory is gone. If unprobed, warm the cache and temporarily return false. Convert to Regular Session/Group
 *  uses this gate and is allowed only when the node's bound worktree directory truly no longer exists. */
export function isWorktreeGone(worktreePath?: string | null): boolean {
  if (!worktreePath) return false;
  const info = peekGitBranchInfo(worktreePath);
  if (info === undefined) {
    prefetchGitBranchInfo(worktreePath);
    return false;
  }
  return !info.isRepo;
}

/** Live labels watch for changes; passive sidebar consumers only populate/read the shared cache. */
export function useGitBranchInfo(path?: string | null, live = true): GitBranchInfo {
  const [info, setInfo] = useState<GitBranchInfo>(() =>
    path && cache.has(path) ? cache.get(path)! : EMPTY,
  );

  useEffect(() => {
    if (!path) {
      setInfo(EMPTY);
      return;
    }
    // Updates arrive through the listener, including the first probe's answer, so a superseded probe
    // never reaches the component.
    const release = track(path, setInfo, live);
    const cached = cache.get(path);
    if (cached) setInfo(cached);
    if (!cached || live) void probe(path);
    return release;
  }, [path, live]);

  return info;
}

/** Linked worktree containing each path, keyed by path; paths outside a linked worktree are absent. Like sidebar
 *  icons this only fills and reads the shared cache, probing uncached paths once, and never watches or polls. */
export function useLinkedWorktrees(paths: string[]): Map<string, string> {
  const key = paths.join("\n");
  const [version, setVersion] = useState(0);
  useEffect(() => {
    if (!key) return;
    const list = key.split("\n");
    const bump = () => setVersion((v) => v + 1);
    const releases = list.map((path) => track(path, bump, false));
    for (const path of list) if (!cache.has(path)) void probe(path);
    return () => { for (const release of releases) release(); };
  }, [key]);
  return useMemo(() => {
    const out = new Map<string, string>();
    if (!key) return out;
    for (const path of key.split("\n")) {
      const info = cache.get(path);
      if (info?.isWorktree && info.worktreePath) out.set(path, info.worktreePath);
    }
    return out;
    // `version` changes whenever a tracked path's cached answer does.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, version]);
}

/** Get only the branch name for compatibility with legacy callers. */
export function useGitBranch(path?: string | null, live = true): string | null {
  return useGitBranchInfo(path, live).branch;
}
