//! Shared server file browser for browser and remote windows without native file dialogs. It browses the
//! server through backend `list_dir` and is shared by SaveAsModal and DirectoryPickerModal, which differ
//! only in their title and footer actions.
//!
//! The UI replaces the old drill-down dialog with:
//! 1. A lazily loaded expandable tree using shared `.file-row` styles and icons, preserving visible context.
//! 2. Quick access to Home, project roots, and recent directories.
//! 3. One path/search field: values beginning with `/` or `~` navigate on Enter; others filter quick entries
//!    and already loaded tree levels only, never recursively search the server.
//! 4. Clickable path breadcrumbs that reset the tree root at any level.
//! 5. Inline folder creation in the current target directory.
//! 6. An always-visible target directory in the footer.
//! SaveAsModal alone handles overwrite confirmation.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Icons from "../components/Icons";
import { useT } from "../i18n";
import { createDir, listDir, type DirEntry } from "../ipc/info";
import { invoke } from "../ipc/transport";
import { useTermStore } from "../store/termStore";

/** Join a directory and child name into a full path. */
export function joinPath(dir: string, name: string): string {
  const trimmed = dir.replace(/[\\/]+$/, "");
  const separator = dir.includes("\\") && !dir.includes("/") ? "\\" : "/";
  return trimmed ? `${trimmed}${separator}${name}` : `${separator}${name}`;
}

/** Return the parent of a POSIX path, with root returning itself. */
function parentOf(path: string): string {
  if (path === "/" || path === "") return "/";
  const trimmed = path.replace(/\/+$/, "");
  const i = trimmed.lastIndexOf("/");
  return i <= 0 ? "/" : trimmed.slice(0, i);
}

/** Use the final path segment as its display name, with root shown as `/`. */
function baseName(path: string): string {
  return path.replace(/\/+$/, "").split("/").filter(Boolean).pop() || "/";
}

interface TreeNode {
  name: string;
  path: string;
  isDir: boolean;
  isHidden: boolean;
  badge?: string | null;
  open?: boolean;
  /** Whether children have been fetched from the backend. */
  loaded?: boolean;
  children?: TreeNode[];
}

/** Convert a backend DirEntry to a tree node with an absolute path. */
function toNode(e: DirEntry, parentPath: string): TreeNode {
  const base = parentPath === "/" ? "" : parentPath;
  return {
    name: e.name,
    path: `${base}/${e.name}`,
    isDir: e.isDir,
    isHidden: e.isHidden,
    badge: e.gitBadge,
    open: false,
    loaded: false,
  };
}

/** Find a tree node by absolute path, used to refresh a directory after creation. */
function findNode(nodes: TreeNode[], path: string): TreeNode | null {
  for (const n of nodes) {
    if (n.path === path) return n;
    if (n.children) {
      const hit = findNode(n.children, path);
      if (hit) return hit;
    }
  }
  return null;
}

/**
 * Whether this node or any loaded descendant name matches the query. Because `children` contains only
 * fetched levels, unopened subtrees intentionally remain outside search scope.
 */
function nodeMatches(node: TreeNode, q: string): boolean {
  if (node.name.toLowerCase().includes(q)) return true;
  return (node.children || []).some((c) => nodeMatches(c, q));
}

/** Split an absolute path into clickable breadcrumbs beginning with `/`. */
function crumbsOf(path: string): { name: string; path: string }[] {
  const out = [{ name: "/", path: "/" }];
  let acc = "";
  for (const seg of path.split("/").filter(Boolean)) {
    acc += `/${seg}`;
    out.push({ name: seg, path: acc });
  }
  return out;
}

/** Recent-directory persistence in localStorage, falling back quietly to empty when unavailable. */
const RECENTS_KEY = "vlxterm.pickerRecents";
function loadRecents(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(RECENTS_KEY) || "[]");
    return Array.isArray(v) ? v.filter((x) => typeof x === "string") : [];
  } catch {
    return [];
  }
}

export interface QuickItem {
  label: string;
  path: string;
  /** Selects a QUICK_ICON and combines with label as the key. */
  kind: "home" | "project" | "recent";
}

/**
 * Browser-state hook managing root, tree contents, target directory, hidden toggle, quick entries, and
 * recents. When the dialog becomes active, reset the tree root to Home.
 */
export function useServerBrowser(active: boolean) {
  const projects = useTermStore((s) => s.projects);
  const [home, setHome] = useState<string>("");
  const [rootPath, setRootPath] = useState<string>("");
  const [roots, setRoots] = useState<TreeNode[] | null>(null); // null means loading.
  const [treeError, setTreeError] = useState(false);
  const [selectedDir, setSelectedDir] = useState<string>("");
  const [showHidden, setShowHidden] = useState(false);
  const [recents, setRecents] = useState<string[]>(() => loadRecents());
  const [query, setQuery] = useState("");
  const [error, setError] = useState(""); // Action error written by save/import dialogs.

  /** Set a directory as both tree root and current target. */
  const setRoot = useCallback((target: string) => {
    setRootPath(target);
    setSelectedDir(target);
    setQuery("");
  }, []);

  // Fetch the first level whenever the tree root changes.
  useEffect(() => {
    if (!rootPath) return;
    let cancelled = false;
    setRoots(null);
    setTreeError(false);
    listDir(rootPath)
      .then((kids) => {
        if (!cancelled) setRoots(kids.map((k) => toNode(k, rootPath)));
      })
      .catch(() => {
        if (!cancelled) {
          setRoots([]);
          setTreeError(true);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [rootPath]);

  // Start at Home whenever the dialog opens.
  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    setError("");
    setQuery("");
    invoke<string | null>("home_dir")
      .then((h) => {
        if (cancelled) return;
        const base = h || "/";
        setHome(base);
        setRoot(base);
      })
      .catch(() => {
        if (!cancelled) {
          setHome("");
          setRoot("/");
        }
      });
    return () => {
      cancelled = true;
    };
  }, [active, setRoot]);

  /** Toggle a directory, fetching children on first expansion and replacing the array to rerender. */
  const toggleDir = useCallback(async (node: TreeNode) => {
    if (node.loaded) {
      node.open = !node.open;
      setRoots((c) => (c ? [...c] : c));
      return;
    }
    node.open = true;
    setRoots((c) => (c ? [...c] : c));
    try {
      const kids = await listDir(node.path);
      node.children = kids.map((k) => toNode(k, node.path));
    } catch {
      node.children = [];
    }
    node.loaded = true;
    setRoots((c) => (c ? [...c] : c));
  }, []);

  /**
   * Reload a directory's immediate children after folder creation. Replace roots when it is the current
   * root; otherwise update the node in place, preserve expansion, and replace the array to rerender.
   */
  const refreshDir = useCallback(
    async (dirPath: string) => {
      const kids = await listDir(dirPath);
      if (dirPath === rootPath) {
        setRoots(kids.map((k) => toNode(k, dirPath)));
        return;
      }
      setRoots((prev) => {
        if (!prev) return prev;
        const node = findNode(prev, dirPath);
        if (!node) return prev;
        node.children = kids.map((k) => toNode(k, dirPath));
        node.loaded = true;
        node.open = true;
        return [...prev];
      });
    },
    [rootPath],
  );

  /**
   * Create a folder under the current target, refresh that level, and select the new directory. Propagate
   * failures so the caller can write `error`.
   */
  const createFolder = useCallback(
    async (name: string) => {
      const parent = selectedDir || rootPath;
      if (!parent) return;
      const path = joinPath(parent, name);
      await createDir(path);
      await refreshDir(parent);
      setSelectedDir(path);
    },
    [selectedDir, rootPath, refreshDir],
  );

  /** Record a recent directory, deduplicated and moved to the front with a limit of six. */
  const pushRecent = useCallback((p: string) => {
    setRecents((prev) => {
      const next = [p, ...prev.filter((x) => x !== p)].slice(0, 6);
      try {
        localStorage.setItem(RECENTS_KEY, JSON.stringify(next));
      } catch {
        /* If localStorage is unavailable, retain it only for this session. */
      }
      return next;
    });
  }, []);

  // Quick entries combine Home, project roots, and recents, deduplicated by path.
  const quick = useMemo<QuickItem[]>(() => {
    const items: QuickItem[] = [];
    const seen = new Set<string>();
    const add = (it: QuickItem) => {
      const k = it.path.replace(/\/+$/, "") || "/";
      if (!seen.has(k)) {
        seen.add(k);
        items.push(it);
      }
    };
    if (home) add({ label: "Home", path: home, kind: "home" });
    for (const p of projects) {
      if (p.rootPath) add({ label: p.name || baseName(p.rootPath), path: p.rootPath, kind: "project" });
    }
    for (const r of recents) add({ label: baseName(r), path: r, kind: "recent" });
    return items;
  }, [home, projects, recents]);

  return {
    rootPath,
    roots,
    treeError,
    selectedDir,
    setSelectedDir,
    setRoot,
    toggleDir,
    createFolder,
    showHidden,
    setShowHidden,
    quick,
    pushRecent,
    query,
    setQuery,
    error,
    setError,
  };
}

export type ServerBrowser = ReturnType<typeof useServerBrowser>;

/** One tree row. Directories expand/select; Save As may click files to fill the filename. */
function Row({
  node,
  depth,
  browser,
  showHidden,
  filter,
  onFileClick,
  selectedName,
}: {
  node: TreeNode;
  depth: number;
  browser: ServerBrowser;
  showHidden: boolean;
  /** Normalized lowercase trimmed query; empty disables filtering. */
  filter: string;
  onFileClick?: (name: string) => void;
  selectedName?: string;
}) {
  const pad = 8 + depth * 13;
  if (node.isDir) {
    const sel = browser.selectedDir === node.path;
    // A node matching the query itself is shown for its own sake: its whole subtree stays browsable and
    // keeps normal expand/collapse. A node shown only because a descendant matches is forced open and
    // pruned to the matching branches. Clearing the query restores plain user expansion state everywhere.
    const selfMatch = filter !== "" && node.name.toLowerCase().includes(filter);
    const childFilter = selfMatch ? "" : filter;
    const open = childFilter ? true : !!node.open;
    let kids = (node.children || []).filter((c) => showHidden || !c.isHidden);
    if (childFilter) kids = kids.filter((c) => nodeMatches(c, childFilter));
    return (
      <div>
        <div
          className={"file-row" + (sel ? " sel" : "") + (node.isHidden ? " hidden-entry" : "")}
          style={{ paddingLeft: pad }}
          onClick={() => {
            browser.setSelectedDir(node.path);
            void browser.toggleDir(node);
          }}
        >
          <span className="tw">{open ? <Icons.chevD size={13} /> : <Icons.chevR size={13} />}</span>
          <span className="ic" style={{ color: "var(--text-dim)" }}>
            {open ? <Icons.folderOpen size={14} /> : <Icons.folder size={14} />}
          </span>
          <span className="nm">{node.name}</span>
          {node.badge && <span className={"gb gb-" + node.badge}>{node.badge}</span>}
        </div>
        {open &&
          kids.map((c) => (
            <Row
              key={c.path}
              node={c}
              depth={depth + 1}
              browser={browser}
              showHidden={showHidden}
              filter={childFilter}
              onFileClick={onFileClick}
              selectedName={selectedName}
            />
          ))}
      </div>
    );
  }
  const selFile =
    onFileClick != null && selectedName != null && node.name === selectedName && browser.selectedDir === parentOf(node.path);
  return (
    <div
      className={"file-row" + (selFile ? " sel" : "") + (node.isHidden ? " hidden-entry" : "")}
      style={{ paddingLeft: pad, cursor: onFileClick ? "pointer" : "default" }}
      onClick={
        onFileClick
          ? () => {
              browser.setSelectedDir(parentOf(node.path));
              onFileClick(node.name);
            }
          : undefined
      }
    >
      <span className="tw leaf" />
      <span className="ic" style={{ color: "var(--text-faint)" }}>
        <Icons.file size={13} />
      </span>
      <span className="nm">{node.name}</span>
      {node.badge && <span className={"gb gb-" + node.badge}>{node.badge}</span>}
    </div>
  );
}

/**
 * Browser body with navigation input, quick entries, root row, directory tree, and target row. Dialogs
 * supply their own title and footer. With `onFileClick`, files fill a Save As filename; without it they
 * are read-only context for directory selection. `selectedName` highlights a matching target-directory file.
 */
export function ServerBrowserView({
  browser,
  onFileClick,
  selectedName,
}: {
  browser: ServerBrowser;
  onFileClick?: (name: string) => void;
  selectedName?: string;
}) {
  const t = useT();
  const q = browser.query.trim().toLowerCase();
  const pathLike = q.includes("/") || q.startsWith("~");
  // Do not filter while typing a path; hiding quick entries and the tree would impede navigation.
  const treeFilter = pathLike ? "" : q;
  const chips = !treeFilter
    ? browser.quick
    : browser.quick.filter(
        (it) => it.label.toLowerCase().includes(treeFilter) || it.path.toLowerCase().includes(treeFilter),
      );

  // Inline new-folder field at the top of the tree, avoiding nested dialogs.
  const [creating, setCreating] = useState(false);
  const [newName, setNewName] = useState("");

  // After changing root, scroll breadcrumbs right so the current level remains visible.
  const crumbBoxRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = crumbBoxRef.current;
    if (el) el.scrollLeft = el.scrollWidth;
  }, [browser.rootPath]);

  const jump = async () => {
    const raw = browser.query.trim();
    if (!raw) return;
    if (raw.includes("/") || raw.startsWith("~")) {
      let target = raw;
      if (raw === "~" || raw.startsWith("~/")) {
        const h = await invoke<string | null>("home_dir").catch(() => null);
        if (h) target = h + raw.slice(1);
      }
      if (target !== "/") target = target.replace(/\/+$/, "");
      browser.setRoot(target);
    } else if (chips[0]) {
      browser.setRoot(chips[0].path);
    }
  };

  const startCreate = () => {
    browser.setError("");
    setNewName("");
    setCreating(true);
  };

  // Close the input before asynchronous creation so the following blur sees `creating=false` and does not
  // interpret the same submission as cancellation.
  const submitCreate = () => {
    const name = newName.trim();
    setCreating(false);
    if (!name) return;
    void browser.createFolder(name).catch((e) => browser.setError(String(e)));
  };

  let visibleRoots = (browser.roots || []).filter((c) => browser.showHidden || !c.isHidden);
  if (treeFilter) visibleRoots = visibleRoots.filter((c) => nodeMatches(c, treeFilter));

  const crumbs = crumbsOf(browser.rootPath);
  const atRoot = !browser.rootPath || browser.rootPath === "/";

  return (
    <>
      <div style={{ flexShrink: 0, position: "relative", padding: "0 16px 8px" }}>
        <span
          style={{
            position: "absolute",
            left: 25,
            top: 8,
            display: "grid",
            placeItems: "center",
            color: "var(--text-faint)",
            pointerEvents: "none",
          }}
        >
          <Icons.search size={13} />
        </span>
        <input
          value={browser.query}
          onChange={(e) => browser.setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void jump();
            if (e.key === "Escape" && browser.query) {
              e.stopPropagation(); // Clear search without letting Escape close the entire dialog.
              browser.setQuery("");
            }
          }}
          placeholder={t("dir.pathPlaceholder")}
          spellCheck={false}
          style={{
            width: "100%",
            boxSizing: "border-box",
            padding: "7px 10px 7px 30px",
            border: "1px solid var(--border)",
            borderRadius: 7,
            background: "var(--bg-0)",
            fontFamily: "var(--font-mono, monospace)",
            fontSize: 12,
            color: "var(--text)",
            outline: "none",
          }}
        />
      </div>

      {chips.length > 0 && (
        <div className="picker-hscroll" style={{ flexShrink: 0, display: "flex", gap: 6, padding: "0 16px 9px" }}>
          {chips.map((it) => {
            const active = browser.rootPath === it.path;
            const Icon = QUICK_ICON[it.kind];
            return (
              <button
                key={it.kind + it.path}
                onClick={() => browser.setRoot(it.path)}
                title={it.path}
                style={{
                  flexShrink: 0,
                  display: "inline-flex",
                  alignItems: "center",
                  gap: 5,
                  maxWidth: 160,
                  height: 26,
                  padding: "0 10px",
                  border: `1px solid ${active ? "var(--accent)" : "var(--border)"}`,
                  borderRadius: 13,
                  background: active ? "var(--accent-soft)" : "transparent",
                  color: active ? "var(--text)" : "var(--text-mid)",
                  fontSize: 12,
                  cursor: "pointer",
                }}
              >
                <span style={{ display: "grid", placeItems: "center", color: "var(--text-faint)" }}>
                  <Icon size={12} />
                </span>
                <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{it.label}</span>
              </button>
            );
          })}
        </div>
      )}

      <div
        style={{
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          gap: 4,
          padding: "0 12px 6px",
        }}
      >
        <button
          onClick={() => browser.setRoot(parentOf(browser.rootPath))}
          disabled={atRoot}
          title={t("dir.up")}
          style={iconBtn(false, atRoot)}
        >
          <Icons.arrowLeft size={13} />
        </button>

        <div
          ref={crumbBoxRef}
          className="picker-hscroll"
          style={{ flex: 1, minWidth: 0, display: "flex", alignItems: "center", gap: 1 }}
        >
          {crumbs.map((c, i) => {
            const last = i === crumbs.length - 1;
            return (
              <span key={c.path} style={{ flexShrink: 0, display: "inline-flex", alignItems: "center" }}>
                {i > 0 && (
                  <span style={{ color: "var(--text-faint)", padding: "0 1px" }}>
                    <Icons.chevR size={11} />
                  </span>
                )}
                <button
                  onClick={() => browser.setRoot(c.path)}
                  title={c.path}
                  style={{
                    padding: "3px 6px",
                    border: "none",
                    borderRadius: 5,
                    background: "transparent",
                    color: last ? "var(--text)" : "var(--text-dim)",
                    fontSize: 12.5,
                    fontWeight: last ? 600 : 400,
                    whiteSpace: "nowrap",
                    cursor: "pointer",
                  }}
                >
                  {c.name}
                </button>
              </span>
            );
          })}
        </div>

        <button
          onClick={startCreate}
          disabled={!browser.selectedDir}
          title={t("dir.newFolder")}
          style={iconBtn(false, !browser.selectedDir)}
        >
          <Icons.folderPlus size={14} />
        </button>
        <button
          onClick={() => browser.setShowHidden((v) => !v)}
          title={t("dir.showHidden")}
          aria-pressed={browser.showHidden}
          style={iconBtn(browser.showHidden, false)}
        >
          {browser.showHidden ? <Icons.eye size={14} /> : <Icons.eyeOff size={14} />}
        </button>
      </div>

      <div style={{ flex: 1, minWidth: 0, minHeight: 0, overflowY: "auto", paddingBottom: 6 }}>
        {creating && (
          <div className="file-row" style={{ paddingLeft: 8, cursor: "default" }}>
            <span className="tw leaf" />
            <span className="ic" style={{ color: "var(--text-dim)" }}>
              <Icons.folder size={14} />
            </span>
            <input
              autoFocus
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              onBlur={submitCreate}
              onKeyDown={(e) => {
                if (e.key === "Enter") submitCreate();
                if (e.key === "Escape") {
                  e.stopPropagation(); // Cancel creation without closing the dialog.
                  setCreating(false);
                }
              }}
              placeholder={t("dir.newFolderPlaceholder")}
              spellCheck={false}
              style={{
                flex: 1,
                minWidth: 0,
                padding: "1px 5px",
                border: "1px solid var(--accent)",
                borderRadius: 4,
                background: "var(--bg-0)",
                color: "var(--text)",
                fontSize: 12.5,
                outline: "none",
              }}
            />
          </div>
        )}

        {browser.roots == null ? (
          <div style={emptyHint}>{t("common.loading")}</div>
        ) : browser.treeError ? (
          <div style={emptyHint}>{t("panel.cantRead")}</div>
        ) : visibleRoots.length === 0 ? (
          <div style={emptyHint}>{treeFilter ? t("dir.noMatch") : t("dir.empty")}</div>
        ) : (
          visibleRoots.map((c) => (
            <Row
              key={c.path}
              node={c}
              depth={0}
              browser={browser}
              showHidden={browser.showHidden}
              filter={treeFilter}
              onFileClick={onFileClick}
              selectedName={selectedName}
            />
          ))
        )}
      </div>

      <div
        style={{
          flexShrink: 0,
          display: "flex",
          alignItems: "center",
          gap: 6,
          padding: "7px 16px",
          borderTop: "1px solid var(--border)",
          fontSize: 11.5,
        }}
      >
        <span style={{ flexShrink: 0, color: "var(--text-dim)" }}>{t("dir.target")}</span>
        <span
          title={browser.selectedDir}
          style={{
            minWidth: 0,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            direction: "rtl", // Preserve the meaningful final levels and truncate long paths from the start.
            textAlign: "left",
            color: "var(--text-mid)",
            fontFamily: "var(--font-mono, monospace)",
          }}
        >
          {browser.selectedDir || "—"}
        </span>
      </div>

      {browser.error && (
        <div style={{ flexShrink: 0, padding: "0 16px 8px", fontSize: 11.5, color: "var(--danger, #ff6b6b)" }}>
          {browser.error}
        </div>
      )}
    </>
  );
}

/** Quick-entry icon by kind, sharing outline icons with the sidebar/files panel instead of emoji. */
const QUICK_ICON = {
  home: Icons.home,
  project: Icons.project,
  recent: Icons.clock,
} as const;

const emptyHint: React.CSSProperties = { padding: 16, color: "var(--text-dim)", fontSize: 12 };

/** Consistently sized square toolbar button for parent, new folder, and hidden-item actions. */
function iconBtn(active: boolean, disabled: boolean): React.CSSProperties {
  return {
    flexShrink: 0,
    display: "grid",
    placeItems: "center",
    width: 26,
    height: 26,
    border: `1px solid ${active ? "var(--accent)" : "var(--border)"}`,
    borderRadius: 6,
    background: active ? "var(--accent-soft)" : "transparent",
    color: active ? "var(--text)" : "var(--text-mid)",
    cursor: disabled ? "default" : "pointer",
    opacity: disabled ? 0.4 : 1,
  };
}

/** Shared responsive dialog-card style that avoids horizontal overflow on mobile and narrow remote views. */
export const cardStyle: React.CSSProperties = {
  width: "min(460px, calc(100vw - 24px))",
  maxHeight: "min(80vh, 620px)",
  display: "flex",
  flexDirection: "column",
  background: "var(--bg-2)",
  border: "1px solid var(--border-strong)",
  borderRadius: 12,
  boxShadow: "var(--shadow)",
  overflow: "hidden",
};

export const ghostBtn: React.CSSProperties = {
  padding: "7px 14px",
  border: "1px solid var(--border)",
  borderRadius: 7,
  background: "transparent",
  color: "var(--text-dim)",
  fontSize: 12.5,
  cursor: "pointer",
};

export function primaryBtn(disabled: boolean): React.CSSProperties {
  return {
    padding: "7px 16px",
    border: "none",
    borderRadius: 7,
    background: "var(--accent)",
    color: "var(--bg-0)",
    fontSize: 12.5,
    fontWeight: 600,
    cursor: disabled ? "default" : "pointer",
    opacity: disabled ? 0.6 : 1,
  };
}
