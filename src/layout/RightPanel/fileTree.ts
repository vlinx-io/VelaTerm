//! Files-tree reconciliation preserves local expansion and lazy children while replacing backend facts.

import type { DirEntry } from "../../ipc/info";

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
