//! Files-tree reconciliation preserves local expansion and lazy children while replacing backend facts.

import type { DirEntry, SearchFileEntry } from "../../ipc/info";

export interface FileNodeT {
  name: string;
  path: string;
  isDir: boolean;
  isHidden: boolean;
  badge?: string | null;
  open?: boolean;
  loaded?: boolean;
  loading?: boolean;
  children?: FileNodeT[];
}

export function toNode(entry: DirEntry, parentPath: string): FileNodeT {
  return {
    name: entry.name,
    path: `${parentPath.replace(/[\\/]$/, "")}/${entry.name}`,
    isDir: entry.isDir,
    isHidden: entry.isHidden,
    badge: entry.gitBadge,
    open: false,
    loaded: false,
  };
}

export function reconcileChildren(node: FileNodeT, entries: DirEntry[]): FileNodeT {
  const previous = new Map(node.children?.map((child) => [child.path, child]));
  const children = entries.map((entry) => {
    const fresh = toNode(entry, node.path);
    const old = previous.get(fresh.path);
    return old && old.isDir === fresh.isDir ? {
      ...old, name: fresh.name, isHidden: fresh.isHidden, badge: fresh.badge,
    } : fresh;
  });
  return { ...node, children, loaded: true, loading: false };
}

export function updateFileNode(root: FileNodeT, path: string, update: (node: FileNodeT) => FileNodeT): FileNodeT {
  if (root.path === path) return update(root);
  if (!root.children) return root;
  const children = root.children.map((child) => updateFileNode(child, path, update));
  return children.some((child, index) => child !== root.children![index]) ? { ...root, children } : root;
}

/**
 * Close every loaded directory below `root`, leaving the root itself open.
 *
 * Collapsing the root too would hide the whole tree and read as a broken panel, so the root is the
 * one node exempted. Loaded children are kept, so reopening a directory needs no refetch.
 */
export function collapseAllBelow(root: FileNodeT): FileNodeT {
  if (!root.children) return root;
  const close = (node: FileNodeT): FileNodeT => {
    const children = node.children?.map(close);
    const changed = children?.some((child, index) => child !== node.children![index]);
    return node.open || changed ? { ...node, open: false, ...(children ? { children } : {}) } : node;
  };
  const children = root.children.map(close);
  return children.some((child, index) => child !== root.children![index]) ? { ...root, children } : root;
}

/** Whether any loaded directory below `root` is currently open, i.e. whether collapsing would do
 *  anything. Used to disable the control rather than offer a button that visibly does nothing. */
export function hasOpenDescendant(root: FileNodeT): boolean {
  return (root.children || []).some((c) => !!c.open || hasOpenDescendant(c));
}

/** Only expanded, already loaded directories need background queries. */
export function visibleDirectories(root: FileNodeT): string[] {
  if (!root.isDir || !root.open) return [];
  return [ ...(root.loaded ? [root.path] : []), ...(root.children ?? []).flatMap(visibleDirectories) ];
}

/**
 * Directories to re-read after a change event: the shown ones among `dirs` (relative to the root,
 * `""` being the root), or every shown one when the event could not list them or touched git's own
 * files, which moves the Git badges of any directory.
 */
export function directoriesToRefresh(root: FileNodeT, dirs: string[] | null, git: boolean): string[] {
  const shown = visibleDirectories(root);
  if (dirs === null || git) return shown;
  const base = root.path.replace(/[\\/]$/, "");
  const wanted = new Set(dirs.map((dir) => (dir ? `${base}/${dir}` : root.path)));
  return shown.filter((path) => wanted.has(path));
}

/** Relative path below `rootPath` using `/` separators; `""` for the root itself. */
export function relativePath(rootPath: string, path: string): string {
  const base = rootPath.replace(/[\\/]$/, "");
  if (path === rootPath || path === base) return "";
  if (path.startsWith(base + "/") || path.startsWith(base + "\\")) {
    return path.slice(base.length + 1).replace(/\\/g, "/");
  }
  return path.replace(/\\/g, "/");
}

/** Match an entry against the normalized query; queries with `/` match relative path segments. */
export function matchesFileQuery(name: string, relPath: string, query: string): boolean {
  const q = query.trim().replace(/\\/g, "/").toLowerCase();
  if (!q) return true;
  if (q.includes("/")) {
    if (q.replace(/\//g, "") === "") return false;
    const relLower = relPath.replace(/\\/g, "/").toLowerCase();
    return relLower.includes(q) || (q.startsWith("/") && `/${relLower}`.includes(q));
  }
  return name.toLowerCase().includes(q);
}

/** Whether `node` or any visible descendant matches `query`. */
export function nodeMatchesQuery(
  node: FileNodeT,
  rootPath: string,
  query: string,
  showHidden: boolean,
): boolean {
  if (!showHidden && node.isHidden && node.path !== rootPath) return false;
  if (node.path !== rootPath && matchesFileQuery(node.name, relativePath(rootPath, node.path), query)) {
    return true;
  }
  return (node.children || []).some((child) => nodeMatchesQuery(child, rootPath, query, showHidden));
}

/** Direct children visible under the active hidden-file and filter rules. */
export function visibleChildren(
  node: FileNodeT,
  rootPath: string,
  showHidden: boolean,
  query: string,
): FileNodeT[] {
  const kids = (node.children || []).filter((child) => showHidden || !child.isHidden);
  if (!query) return kids;
  return kids.filter((child) => nodeMatchesQuery(child, rootPath, query, showHidden));
}

/**
 * Whether `node` is expanded in the current view.
 *
 * When filtering, directories with matching descendants start expanded so matches appear immediately,
 * while clicking a directory adds or removes its path in `filterCollapsed` so parents can always be
 * collapsed and reopened without mutating the underlying tree's normal `open` state.
 */
export function isNodeOpen(
  node: FileNodeT,
  rootPath: string,
  query: string,
  filterCollapsed: ReadonlySet<string>,
  visibleChildCount: number,
): boolean {
  if (!node.isDir) return false;
  if (!query) return !!node.open;
  if (node.path === rootPath) return true;
  if (node.loading) return true;
  if (visibleChildCount === 0) return false;
  return !filterCollapsed.has(node.path);
}

/** Whether a directory can be toggled open/closed while filtering. */
export function canExpandInFilter(node: FileNodeT, visibleChildCount: number): boolean {
  return node.isDir && (visibleChildCount > 0 || !node.loaded);
}

/** Paths of every currently expanded directory below `rootPath` in the filtered tree. */
export function openFilterDescendants(
  root: FileNodeT,
  rootPath: string,
  showHidden: boolean,
  query: string,
  filterCollapsed: ReadonlySet<string>,
): string[] {
  const out: string[] = [];
  const walk = (node: FileNodeT) => {
    if (!node.isDir) return;
    const kids = visibleChildren(node, rootPath, showHidden, query);
    if (!isNodeOpen(node, rootPath, query, filterCollapsed, kids.length)) return;
    if (node.path !== rootPath) out.push(node.path);
    kids.forEach(walk);
  };
  walk(root);
  return out;
}

/** Flatten visible rows in render order for keyboard navigation. */
export function flattenVisibleNodes(
  root: FileNodeT,
  rootPath: string,
  showHidden: boolean,
  query: string,
  filterCollapsed: ReadonlySet<string>,
): FileNodeT[] {
  const out: FileNodeT[] = [];
  const walk = (node: FileNodeT) => {
    out.push(node);
    if (!node.isDir) return;
    const kids = visibleChildren(node, rootPath, showHidden, query);
    if (!isNodeOpen(node, rootPath, query, filterCollapsed, kids.length)) return;
    kids.forEach(walk);
  };
  walk(root);
  return out;
}

/** Build a sorted `FileNodeT` tree from flat recursive search entries returned by `search_files`. */
export function buildSearchTree(
  rootPath: string,
  rootName: string,
  rootBadge: string | null | undefined,
  entries: SearchFileEntry[],
): FileNodeT {
  const base = rootPath.replace(/[\\/]$/, "");
  const root: FileNodeT = {
    name: rootName,
    path: rootPath,
    isDir: true,
    isHidden: false,
    badge: rootBadge ?? null,
    open: true,
    loaded: true,
    children: [],
  };
  const nodes = new Map<string, FileNodeT>([["", root]]);
  const ensureDir = (rel: string): FileNodeT => {
    const existing = nodes.get(rel);
    if (existing) return existing;
    const slash = rel.lastIndexOf("/");
    const parentRel = slash >= 0 ? rel.slice(0, slash) : "";
    const name = slash >= 0 ? rel.slice(slash + 1) : rel;
    const parent = ensureDir(parentRel);
    const dir: FileNodeT = {
      name,
      path: `${base}/${rel}`,
      isDir: true,
      isHidden: name.startsWith("."),
      badge: null,
      open: false,
      loaded: true,
      children: [],
    };
    nodes.set(rel, dir);
    parent.children!.push(dir);
    return dir;
  };

  for (const entry of entries) {
    if (!entry.path) continue;
    if (entry.isDir) {
      const dir = ensureDir(entry.path);
      dir.isHidden = entry.isHidden;
      dir.badge = entry.gitBadge;
    } else {
      const slash = entry.path.lastIndexOf("/");
      const parentRel = slash >= 0 ? entry.path.slice(0, slash) : "";
      const name = slash >= 0 ? entry.path.slice(slash + 1) : entry.path;
      const parent = ensureDir(parentRel);
      if (!nodes.has(entry.path)) {
        const file: FileNodeT = {
          name,
          path: `${base}/${entry.path}`,
          isDir: false,
          isHidden: entry.isHidden,
          badge: entry.gitBadge,
          open: false,
          loaded: false,
        };
        nodes.set(entry.path, file);
        parent.children!.push(file);
      }
    }
  }

  const sortChildren = (node: FileNodeT) => {
    if (!node.children) return;
    node.children.sort((a, b) => {
      if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
      const al = a.name.toLowerCase();
      const bl = b.name.toLowerCase();
      return al < bl ? -1 : al > bl ? 1 : 0;
    });
    node.children.forEach(sortChildren);
  };
  sortChildren(root);
  return root;
}

const IGNORED_WATCH_SEGMENTS = new Set([
  ".git", "node_modules", ".hg", ".svn", ".vlx-worktrees",
  "__pycache__", ".pytest_cache", ".mypy_cache", ".ruff_cache",
  ".next", ".nuxt", ".turbo", ".parcel-cache", ".cache",
  ".venv", "venv", ".gradle", ".tox", ".cargo",
]);

/** Whether an `fs://changed` event warrants re-running an active recursive file filter. */
export function shouldRefreshSearch(dirs: string[] | null, git: boolean, showHidden: boolean): boolean {
  if (git || dirs === null) return true;
  return dirs.some((dir) => {
    if (!dir) return true;
    const segments = dir.split("/").filter(Boolean);
    return !segments.some((seg) => IGNORED_WATCH_SEGMENTS.has(seg) || (!showHidden && seg.startsWith(".")));
  });
}
