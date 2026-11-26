//! Right-side Files tab: lazily loads the real directory tree from the session cwd, with Git badges,
//! hidden-item filtering, search, file preview, and create/edit/delete context actions. Extracted from
//! RightPanel; FileNodeT, FileRow, ConfirmModal, and related helpers remain private to this tab.

import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import Icons from "../../components/Icons";
import { Backdrop } from "../../components/Backdrop";
import { fileIcon } from "./fileIcons";
import { highlight } from "./highlight";
import {
  buildSearchTree,
  canExpandInFilter,
  collapseAllBelow,
  directoriesToRefresh,
  flattenVisibleNodes,
  hasOpenDescendant,
  isNodeOpen,
  openFilterDescendants,
  reconcileChildren,
  shouldRefreshSearch,
  toNode,
  updateFileNode,
  visibleChildren,
  visibleDirectories,
  type FileNodeT,
} from "./fileTree";
import {
  clearCutClipboard,
  copyFilesToClipboard,
  cutFilesToClipboard,
  finishCutPaste,
  getFileClipboardSnapshot,
  isPathInList,
  normalizeSourcePaths,
  notifyTextCopied,
  parentPathOf,
  rangeBetweenPaths,
  readFilesForPaste,
  refreshFileClipboardStatus,
  sameFilePath,
  subscribeFileClipboard,
  targetPasteDir,
} from "./fileClipboard";
import { ContextMenu, type MenuItem } from "../../components/ContextMenu";
import { FormModal } from "../../components/FormModal";
import { hasMod } from "../../hooks/shortcutRegistry";
import { useT } from "../../i18n";
import {
  copyIntoDir,
  copyText,
  createDir,
  createFile,
  deletePath,
  fsWatch,
  listDir,
  pasteFiles,
  pasteFilesCheck,
  readFilePreview,
  renamePath,
  revealPath,
  searchFiles,
  type FileClipboardMode,
  type FilePreview,
  type PasteCheckItem,
  type PasteConflictResolution,
} from "../../ipc/info";
import {
  cancelTransfer,
  clearFinishedTransfers,
  getTransfers,
  startDownload,
  startUpload,
  subscribeTransfers,
  type Transfer,
} from "../../ipc/transfer";
import { env, platform } from "../../platform";
import { safeError } from "../../ipc/diagnosticSafety";
import { baseName, detectFlavor } from "../../remote/serverPath";
import { useTermStore } from "../../store/termStore";
import { onFsChanged } from "../../ipc/events";
/* ===================== Files: real tree, preview, and one-level lazy loading ===================== */

/** Find a tree node by path to locate directories that need refreshing. */
function findNodeByPath(root: FileNodeT | null, path: string): FileNodeT | null {
  if (!root) return null;
  if (sameFilePath(root.path, path)) return root;
  for (const c of root.children || []) {
    const f = findNodeByPath(c, path);
    if (f) return f;
  }
  return null;
}

/** Render project outline icons with muted type colors from fileIcons.tsx. Strokes use currentColor,
 * and directories switch between folder and folderOpen according to `open`. */
function renderFileIcon(name: string, isDir: boolean, open = false) {
  const { icon, color } = fileIcon(name, isDir, open);
  const Cmp = Icons[icon] ?? Icons.file;
  return <Cmp size={isDir ? 14 : 13} style={{ color }} />;
}

/**
 * Files of a drop for upload. Uploading a folder would mean walking its entries recursively, which the transfer
 * path does not do, so folders are separated out and reported rather than failing later as unreadable files.
 */
function filesFromDrop(transfer: DataTransfer): { files: File[]; skippedFolders: boolean } {
  const items = Array.from(transfer.items ?? []);
  const files: File[] = [];
  let skippedFolders = false;
  for (const item of items) {
    if (item.webkitGetAsEntry?.()?.isDirectory) {
      skippedFolders = true;
      continue;
    }
    const f = item.getAsFile();
    if (f) files.push(f);
  }
  // Fall back to the flat list where DataTransferItem is unavailable; folders then fail per file.
  if (!items.length) files.push(...Array.from(transfer.files));
  return { files, skippedFolders };
}

function FileRow({
  node,
  rootPath,
  depth,
  selPaths,
  cutPaths,
  ancestorCut = false,
  focusPath,
  showHidden,
  filter,
  filterCollapsed,
  onFile,
  onOpen,
  onDir,
  onContext,
  onDropFiles,
}: {
  node: FileNodeT;
  rootPath: string;
  depth: number;
  selPaths: string[];
  cutPaths: string[];
  ancestorCut?: boolean;
  focusPath: string | null;
  showHidden: boolean;
  /** Lowercase trimmed query; shows matching items and their ancestors while respecting user collapse toggles. */
  filter: string;
  filterCollapsed: ReadonlySet<string>;
  onFile: (n: FileNodeT, e?: React.MouseEvent) => void;
  onOpen: (n: FileNodeT) => void;
  onDir: (n: FileNodeT, e?: React.MouseEvent) => void;
  onContext: (e: React.MouseEvent, n: FileNodeT) => void;
  /** An external file drop onto this row; null disables the drop target. */
  onDropFiles: ((n: FileNodeT, transfer: DataTransfer) => void) | null;
}) {
  const t = useT();
  // Highlight the row under an external file drag. Tracked per row so nested directories do not all light up.
  const [dropOver, setDropOver] = useState(false);
  // A drag carrying files comes from the OS; internal drags (tab and session reordering) carry other types only.
  const dragProps = onDropFiles
    ? {
        onDragOver: (e: React.DragEvent) => {
          if (!e.dataTransfer.types.includes("Files")) return;
          e.preventDefault();
          e.stopPropagation();
          e.dataTransfer.dropEffect = "copy" as const;
          setDropOver(true);
        },
        onDragLeave: () => setDropOver(false),
        onDrop: (e: React.DragEvent) => {
          if (!e.dataTransfer.types.includes("Files")) return;
          e.preventDefault();
          e.stopPropagation();
          setDropOver(false);
          onDropFiles(node, e.dataTransfer);
        },
      }
    : {};
  const dropCls = dropOver ? " drop-target" : "";
  // Each depth gets a 13px indentation cell with a subtle `.ind` guide for legible nesting.
  const inds =
    depth > 0 ? (
      <span className="inds">
        {Array.from({ length: depth }, (_, i) => (
          <i key={i} className="ind" />
        ))}
      </span>
    ) : null;
  const focusCls = focusPath && sameFilePath(focusPath, node.path) ? " kbd-focus" : "";
  const selected = isPathInList(selPaths, node.path);
  const isCut = ancestorCut || isPathInList(cutPaths, node.path);
  const selCls = selected ? " sel" : "";
  const cutCls = isCut ? " cut-entry" : "";
  if (node.isDir) {
    // When hidden items are off, omit a dot-prefixed directory and its entire subtree.
    const kids = visibleChildren(node, rootPath, showHidden, filter);
    const open = isNodeOpen(node, rootPath, filter, filterCollapsed, kids.length);
    const expandable = !filter || node.path === rootPath || canExpandInFilter(node, kids.length);
    return (
      <div>
        <div
          className={"file-row" + selCls + (node.isHidden ? " hidden-entry" : "") + cutCls + focusCls + dropCls}
          onClick={(e) => onDir(node, e)}
          onContextMenu={(e) => onContext(e, node)}
          {...dragProps}
        >
          {inds}
          <span className={"tw" + (expandable ? "" : " leaf")}>
            {open ? <Icons.chevD size={13} /> : <Icons.chevR size={13} />}
          </span>
          <span className="ic">{renderFileIcon(node.name, true, open)}</span>
          <span className="nm">{node.name}</span>
          {node.badge && <span className={"gb gb-" + node.badge}>{node.badge}</span>}
        </div>
        {open &&
          (node.loading ? (
            // While loading children, show an indented spinner row instead of an empty expansion.
            <div className="file-row loading-row">
              <span className="inds">
                {Array.from({ length: depth + 1 }, (_, i) => (
                  <i key={i} className="ind" />
                ))}
              </span>
              <span className="tw">
                <span className="spin" style={{ display: "inline-flex" }}>
                  <Icons.restart size={12} />
                </span>
              </span>
              <span className="nm" style={{ color: "var(--text-faint)" }}>
                {t("common.loading")}
              </span>
            </div>
          ) : (
            kids.map((c) => (
              <FileRow
                key={c.path}
                node={c}
                rootPath={rootPath}
                depth={depth + 1}
                selPaths={selPaths}
                cutPaths={cutPaths}
                ancestorCut={isCut}
                focusPath={focusPath}
                showHidden={showHidden}
                filter={filter}
                filterCollapsed={filterCollapsed}
                onFile={onFile}
                onOpen={onOpen}
                onDir={onDir}
                onContext={onContext}
                onDropFiles={onDropFiles}
              />
            ))
          ))}
      </div>
    );
  }
  return (
    <div
      className={
        "file-row" +
        selCls +
        (node.isHidden ? " hidden-entry" : "") +
        cutCls +
        focusCls +
        dropCls
      }
      title={t("files.dblClickOpen")}
      onClick={(e) => onFile(node, e)}
      onDoubleClick={() => onOpen(node)}
      onContextMenu={(e) => onContext(e, node)}
      {...dragProps}
    >
      {inds}
      <span className="tw leaf" />
      <span className="ic">{renderFileIcon(node.name, false)}</span>
      <span className="nm">{node.name}</span>
      {node.badge && <span className={"gb gb-" + node.badge}>{node.badge}</span>}
    </div>
  );
}

/** True when `list_dir` or `search_files` failed because the target directory itself no longer exists. */
export function isMissingDirectoryError(err: unknown): boolean {
  const msg = String(err);
  return /No such file or directory|Not a directory|cannot find the (?:path|file) specified|\(os error (?:2|3|20|267)\)/i.test(msg);
}

export function FilesTab({
  rootPath,
  rootName,
  onMissingRoot,
}: {
  rootPath: string | null;
  rootName: string | null;
  /** Invoked instead of surfacing an error when `rootPath` no longer exists and a fallback root is available. */
  onMissingRoot?: (missingPath: string) => void;
}) {
  const t = useT();
  const onMissingRootRef = useRef(onMissingRoot);
  onMissingRootRef.current = onMissingRoot;
  const [root, setRoot] = useState<FileNodeT | null>(null);
  const rootRef = useRef(root);
  rootRef.current = root;
  const treeGeneration = useRef(0);
  const treeRevision = useRef(0);
  const [selPaths, setSelPaths] = useState<string[]>([]);
  const anchorPath = useRef<string | null>(null);
  const [preview, setPreview] = useState<FilePreview | null>(null);
  // Preview is explicit: clicks only select; the context-menu Preview action loads the bottom pane.
  // previewNode is independent of selection, and null hides the preview pane.
  const [previewNode, setPreviewNode] = useState<FileNodeT | null>(null);
  // Resizable preview-pane height in pixels; the flex tree consumes the remaining space.
  const [previewH, setPreviewH] = useState(220);
  // Hide dotfiles by default, like system file browsers. The header toggle is in-memory only.
  const [showHidden, setShowHidden] = useState(false);
  // Context menu state; creation targets the clicked node's containing directory.
  const [menu, setMenu] = useState<{ x: number; y: number; node: FileNodeT } | null>(null);
  // Create/rename dialog state. Create within `dir`; rename `node` and refresh its containing `dir`.
  const [dialog, setDialog] = useState<
    { mode: "newFile" | "newDir" | "rename"; dir: FileNodeT; node?: FileNodeT } | null
  >(null);
  // Delete confirmation state for single or multi-select removal.
  const [del, setDel] = useState<{ paths: string[]; label: string } | null>(null);
  // Conflict resolution prompt shown before executing a paste that collides with existing items.
  const [conflictPrompt, setConflictPrompt] = useState<{
    dirPath: string;
    sources: string[];
    mode: FileClipboardMode;
    sequence?: number;
    conflicts: PasteCheckItem[];
    index: number;
    resolutions: Record<string, PasteConflictResolution>;
  } | null>(null);
  const pastingRef = useRef(false);
  const lastKeyClipboardRef = useRef<{ op: "copy" | "cut" | "paste"; at: number } | null>(null);
  const clipboard = useSyncExternalStore(subscribeFileClipboard, getFileClipboardSnapshot);
  // Dismissible operation error such as a duplicate name or insufficient permission.
  const [err, setErr] = useState<string | null>(null);
  const [refreshErr, setRefreshErr] = useState<string | null>(null);
  // Quick filter state: filterOn controls visibility; q is the normalized value passed to the tree.
  const [filterOn, setFilterOn] = useState(false);
  const [filter, setFilter] = useState("");
  // Directories manually collapsed while a filter is active (kept separate from `root`'s `open` flags).
  const [filterCollapsed, setFilterCollapsed] = useState<Set<string>>(() => new Set());
  // Latest recursive backend search result for the active query.
  const [searchState, setSearchState] = useState<{
    rootPath: string;
    showHidden: boolean;
    query: string;
    root: FileNodeT;
    truncated: boolean;
  } | null>(null);
  const [searching, setSearching] = useState(false);
  const searchGen = useRef(0);
  // Keyboard focus row, independent of selected preview; focusing a file also previews it.
  const [focusPath, setFocusPath] = useState<string | null>(null);
  // File transfer is only meaningful when the server is a different machine from the browser: on the desktop
  // the "server" is this machine, where Show in File Manager already does the job.
  const canTransfer = env.isBrowser;
  // Dropping files in works everywhere: browsers upload them, desktop windows copy them from this machine.
  const canDropFiles = canTransfer || env.isTauri || env.isElectron;
  const transfers = useSyncExternalStore(subscribeTransfers, getTransfers);
  // Hidden picker for the upload button, plus the directory the pending pick targets.
  const uploadInputRef = useRef<HTMLInputElement>(null);
  const uploadDirRef = useRef<string | null>(null);
  const treeRef = useRef<HTMLDivElement>(null);
  // Container for the tree and preview, whose height bounds preview resizing.
  const wrapRef = useRef<HTMLDivElement>(null);
  const q = filter.trim().toLowerCase();
  const qRef = useRef(q);
  qRef.current = q;
  const showHiddenRef = useRef(showHidden);
  showHiddenRef.current = showHidden;

  const runSearch = async (targetRoot: string, queryText: string, includeHidden: boolean) => {
    if (!targetRoot || !queryText) return;
    const gen = ++searchGen.current;
    setSearching(true);
    try {
      const res = await searchFiles(targetRoot, queryText, includeHidden);
      if (gen !== searchGen.current) return;
      if (rootRef.current?.path !== targetRoot || qRef.current !== queryText || showHiddenRef.current !== includeHidden) {
        return;
      }
      const currentRoot = rootRef.current;
      const name = currentRoot?.name || rootName || targetRoot.split("/").filter(Boolean).pop() || targetRoot;
      const tree = buildSearchTree(targetRoot, name, currentRoot?.badge, res.entries);
      setSearchState({
        rootPath: targetRoot,
        showHidden: includeHidden,
        query: queryText,
        root: tree,
        truncated: res.truncated,
      });
      setRefreshErr(null);
    } catch (e) {
      if (gen !== searchGen.current) return;
      if (isMissingDirectoryError(e) && onMissingRootRef.current) {
        onMissingRootRef.current(targetRoot);
        return;
      }
      setRefreshErr(String(e));
    } finally {
      if (gen === searchGen.current) setSearching(false);
    }
  };
  const runSearchRef = useRef(runSearch);
  runSearchRef.current = runSearch;

  // Rebuild the root and load its first level when the session root changes.
  useEffect(() => {
    treeGeneration.current++;
    searchGen.current++;
    setSelPaths([]);
    anchorPath.current = null;
    setConflictPrompt(null);
    setPreview(null);
    setPreviewNode(null);
    setErr(null);
    setRefreshErr(null);
    // A filter belongs to the tree it was typed for; don't carry it into another project's root.
    setFilter("");
    setFilterOn(false);
    setFilterCollapsed(new Set());
    setSearchState(null);
    setSearching(false);
    setFocusPath(null);
    if (!rootPath) {
      setRoot(null);
      return;
    }
    const node: FileNodeT = {
      name: rootName || rootPath.split("/").filter(Boolean).pop() || rootPath,
      path: rootPath,
      isDir: true,
      isHidden: false,
      open: true,
      loaded: false,
      loading: true,
    };
    setRoot(node);
    let cancelled = false;
    listDir(rootPath)
      .then((kids) => {
        if (cancelled) return;
        node.children = kids.map((k) => toNode(k, rootPath));
        node.loaded = true;
        node.loading = false;
        setRoot({ ...node });
      })
      .catch((e) => {
        if (cancelled) return;
        if (isMissingDirectoryError(e) && onMissingRootRef.current) {
          onMissingRootRef.current(rootPath);
          return;
        }
        setRefreshErr(String(e));
        node.children = [];
        node.loaded = true;
        node.loading = false;
        setRoot({ ...node });
      });
    return () => {
      cancelled = true;
    };
  }, [rootPath, rootName]);

  // Reset manual filter collapses whenever the root or query changes so new search matches start expanded.
  useEffect(() => {
    setFilterCollapsed(new Set());
  }, [rootPath, q]);

  // Run debounced recursive backend filtering whenever the query, root, or hidden toggle changes.
  useEffect(() => {
    if (!rootPath || !q) {
      searchGen.current++;
      setSearchState(null);
      setSearching(false);
      return;
    }
    setSearching(true);
    const timer = setTimeout(() => {
      void runSearchRef.current(rootPath, q, showHidden);
    }, 150);
    return () => clearTimeout(timer);
  }, [rootPath, q, showHidden]);

  // Refresh visible directory facts without replacing expansion, selection, or loaded descendants.
  //
  // Where the backend can watch the directory (macOS, Windows), a change event re-reads just the
  // shown directories it names, and a full re-read runs only every 30 seconds as a safety net that
  // also renews the watch lease. Elsewhere, or until the lease answers, every shown directory is
  // re-read every 5 seconds.
  useEffect(() => {
    let cancelled = false;
    let running = false;
    // Directories named by events that arrived while a read was running; read right after it.
    let queued: Set<string> | null = null;
    const sample = async (only?: string[]) => {
      const current = rootRef.current;
      if (cancelled || document.hidden || !current || current.path !== rootPath) return;
      const paths = only ?? visibleDirectories(current);
      if (!paths.length) return;
      if (running) {
        queued = new Set([...(queued ?? []), ...paths]);
        return;
      }
      running = true;
      const revision = treeRevision.current;
      try {
        const results = await Promise.allSettled(paths.map((path) => listDir(path)));
        if (cancelled || revision !== treeRevision.current) return;
        const rootIdx = rootPath ? paths.indexOf(rootPath) : -1;
        const rootResult = rootIdx >= 0 ? results[rootIdx] : undefined;
        if (
          rootPath &&
          rootResult?.status === "rejected" &&
          isMissingDirectoryError(rootResult.reason) &&
          onMissingRootRef.current
        ) {
          onMissingRootRef.current(rootPath);
          return;
        }
        setRoot((latest) => {
          if (!latest || latest.path !== rootPath) return latest;
          return results.reduce((tree, result, index) => result.status === "fulfilled"
            ? updateFileNode(tree, paths[index], (node) => reconcileChildren(node, result.value)) : tree, latest);
        });
        const failure = results.find((result) => result.status === "rejected");
        setRefreshErr(failure?.status === "rejected" ? String(failure.reason) : null);
      } finally {
        running = false;
        if (queued && !cancelled) {
          const next = [...queued];
          queued = null;
          void sample(next);
        }
      }
    };
    const refresh = () => {
      void sample();
      if (!document.hidden) void refreshFileClipboardStatus();
      if (rootPath && qRef.current) {
        void runSearchRef.current(rootPath, qRef.current, showHiddenRef.current);
      }
    };

    let watchedRoot: string | null = null;
    let lastFull = Date.now();
    const renew = () => {
      if (!rootPath) return;
      void fsWatch(rootPath)
        .then((info) => { if (!cancelled) watchedRoot = info.supported ? info.root : null; })
        .catch(() => { if (!cancelled) watchedRoot = null; });
    };
    renew();
    void refreshFileClipboardStatus();
    // Events within a short window are merged so a build writing many files costs one read per
    // directory, not one per file.
    let pending: { dirs: Set<string> | null; git: boolean } | null = null;
    let debounce: ReturnType<typeof setTimeout> | null = null;
    const unlisten = onFsChanged(({ root, dirs, git }) => {
      if (cancelled || root !== watchedRoot) return;
      pending = {
        dirs: dirs === null || pending?.dirs === null ? null : new Set([...(pending?.dirs ?? []), ...dirs]),
        git: git || Boolean(pending?.git),
      };
      if (debounce) return;
      debounce = setTimeout(() => {
        debounce = null;
        const batch = pending;
        pending = null;
        const current = rootRef.current;
        if (!batch || !current) return;
        const batchDirs = batch.dirs && [...batch.dirs];
        const paths = directoriesToRefresh(current, batchDirs, batch.git);
        if (paths.length) void sample(paths);
        if (rootPath && qRef.current && shouldRefreshSearch(batchDirs, batch.git, showHiddenRef.current)) {
          void runSearchRef.current(rootPath, qRef.current, showHiddenRef.current);
        }
      }, 150);
    });
    const timer = setInterval(() => {
      if (watchedRoot && Date.now() - lastFull < 30_000 - 100) return;
      lastFull = Date.now();
      if (watchedRoot) renew();
      refresh();
    }, 5000);
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", refresh);
    return () => {
      cancelled = true;
      if (debounce) clearTimeout(debounce);
      void unlisten.then((stop) => stop());
      clearInterval(timer);
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", refresh);
    };
  }, [rootPath, rootName]);

  // Return selectable (non-root) visible node paths in top-to-bottom display order for range selection.
  const visibleSelectablePaths = (): string[] => {
    const activeTree = displayRoot ?? root;
    if (!activeTree) return [];
    return flattenVisibleNodes(activeTree, activeTree.path, showHidden, q, filterCollapsed)
      .filter((n) => !sameFilePath(n.path, activeTree.path))
      .map((n) => n.path);
  };

  // Update tree selection from a pointer click; returns true when modifiers consumed the click.
  const applyClickSelection = (node: FileNodeT, e?: React.MouseEvent): boolean => {
    const activeTree = displayRoot ?? root;
    const isRootNode = !!activeTree && sameFilePath(node.path, activeTree.path);
    setFocusPath(node.path);
    if (isRootNode) {
      if (e && (hasMod(e) || e.shiftKey)) return true;
      setSelPaths([]);
      anchorPath.current = null;
      return false;
    }
    if (e?.shiftKey) {
      const order = visibleSelectablePaths();
      const anchor = anchorPath.current;
      const range = anchor && isPathInList(order, anchor)
        ? rangeBetweenPaths(order, anchor, node.path)
        : [node.path];
      setSelPaths(range);
      if (!anchor || !isPathInList(order, anchor)) {
        anchorPath.current = node.path;
      }
      return true;
    }
    if (e && hasMod(e)) {
      setSelPaths((prev) =>
        isPathInList(prev, node.path)
          ? prev.filter((p) => !sameFilePath(p, node.path))
          : [...prev, node.path],
      );
      anchorPath.current = node.path;
      return true;
    }
    setSelPaths([node.path]);
    anchorPath.current = node.path;
    return false;
  };

  // Toggle a directory, fetching its children before the first expansion (or toggling filterCollapsed when filtering).
  const onDir = async (node: FileNodeT, e?: React.MouseEvent) => {
    if (applyClickSelection(node, e)) return;
    if (q && root) {
      if (node.path === root.path) return;
      if (node.loaded) {
        const kids = visibleChildren(node, root.path, showHidden, q);
        if (!kids.length) return;
        setFilterCollapsed((prev) => {
          const next = new Set(prev);
          if (next.has(node.path)) next.delete(node.path);
          else next.add(node.path);
          return next;
        });
        return;
      }
      const generation = treeGeneration.current;
      treeRevision.current++;
      setRoot((r) => r ? updateFileNode(r, node.path, (current) => ({ ...current, loading: true })) : r);
      try {
        const kids = await listDir(node.path);
        if (generation !== treeGeneration.current) return;
        setRoot((r) => r ? updateFileNode(r, node.path, (current) => reconcileChildren(current, kids)) : r);
      } catch (err) {
        if (generation !== treeGeneration.current) return;
        setErr(String(err));
        setRoot((r) => r ? updateFileNode(r, node.path, (current) => ({ ...current, loading: false })) : r);
      }
      return;
    }
    if (node.loaded) {
      setRoot((r) => r ? updateFileNode(r, node.path, (current) => ({ ...current, open: !current.open })) : r);
      return;
    }
    const generation = treeGeneration.current;
    treeRevision.current++;
    setRoot((r) => r ? updateFileNode(r, node.path, (current) => ({ ...current, loading: true, open: true })) : r);
    try {
      const kids = await listDir(node.path);
      if (generation !== treeGeneration.current) return;
      setRoot((r) => r ? updateFileNode(r, node.path, (current) => reconcileChildren(current, kids)) : r);
    } catch (err) {
      if (generation !== treeGeneration.current) return;
      setErr(String(err));
      setRoot((r) => r ? updateFileNode(r, node.path, (current) => ({ ...current, loading: false })) : r);
    }
  };

  // Clicking a file selects it without previewing; Preview is an explicit context-menu action.
  const onFile = (node: FileNodeT, e?: React.MouseEvent) => {
    applyClickSelection(node, e);
  };

  // Preview selects a file and loads its contents into the bottom pane.
  const doPreview = async (node: FileNodeT) => {
    if (node.isDir) return;
    setSelPaths([node.path]);
    anchorPath.current = node.path;
    setFocusPath(node.path);
    setPreviewNode(node);
    setPreview(null);
    try {
      setPreview(await readFilePreview(node.path));
    } catch {
      setPreview({ content: t("panel.cantRead"), truncated: false, binary: false });
    }
  };

  // Close the preview pane.
  const closePreview = () => {
    setPreviewNode(null);
    setPreview(null);
  };

  // Resize preview vertically, clamping its bounds to the container height.
  const startPreviewResize = (e: React.MouseEvent) => {
    e.preventDefault();
    let last = e.clientY;
    const onMove = (ev: MouseEvent) => {
      const dy = ev.clientY - last;
      last = ev.clientY;
      setPreviewH((h) => {
        const cap = (wrapRef.current?.clientHeight ?? 600) - 160;
        const max = cap > 120 ? cap : 120;
        return Math.max(80, Math.min(max, h - dy));
      });
    };
    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      document.body.style.cursor = "";
    };
    document.body.style.cursor = "row-resize";
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  };

  // Double-click a file to open it in the center document editor.
  const onOpen = (node: FileNodeT) => {
    if (!node.isDir) useTermStore.getState().openDocTab(node.path);
  };

  // Reload a batch of directories after mutations, expanding only explicit targets such as a paste destination.
  const reloadDirs = async (dirPaths: string[], expandTargets: string[] = []) => {
    const generation = treeGeneration.current;
    treeRevision.current++;
    const uniqueDirs: string[] = [];
    for (const dirPath of dirPaths) {
      if (!dirPath || isPathInList(uniqueDirs, dirPath)) continue;
      uniqueDirs.push(dirPath);
    }
    if (!uniqueDirs.length) return;
    try {
      const results = await Promise.allSettled(uniqueDirs.map((dirPath) => listDir(dirPath)));
      if (generation !== treeGeneration.current) return;
      setRoot((r) => {
        if (!r) return r;
        let next = r;
        results.forEach((result, index) => {
          if (result.status !== "fulfilled") return;
          const dirPath = uniqueDirs[index];
          const forceOpen = isPathInList(expandTargets, dirPath);
          const target = findNodeByPath(next, dirPath);
          if (!target) return;
          // Preserve collapsed source parents on external cuts unless expansion was requested.
          if (!forceOpen && !target.loaded) return;
          next = updateFileNode(next, target.path, (current) => ({
            ...reconcileChildren(current, result.value),
            open: forceOpen ? true : current.open,
          }));
        });
        return next;
      });
      const rejected = results.find((result) => result.status === "rejected");
      if (rejected && rejected.status === "rejected") {
        setErr(String(rejected.reason));
      }
      if (rootPath && qRef.current) {
        await runSearchRef.current(rootPath, qRef.current, showHiddenRef.current);
      }
    } catch (e) {
      if (generation === treeGeneration.current) setErr(String(e));
    }
  };

  // Reload a directory's immediate children after mutation while keeping it expanded.
  const reloadDir = async (dirPath: string) => {
    await reloadDirs([dirPath], [dirPath]);
  };

  // Upload one at a time rather than in parallel: several concurrent chunk streams on one connection would
  // compete with terminal traffic without finishing any file sooner.
  const uploadInto = async (dirPath: string, files: File[], skippedFolders = false) => {
    setErr(skippedFolders ? t("transfer.foldersUnsupported") : null);
    let landed = false;
    for (const f of files) {
      landed = (await startUpload(f, dirPath)) || landed;
    }
    if (!landed) return;
    await reloadDir(dirPath);
  };

  // Copy files and folders dropped from the OS file manager on this machine; the backend never overwrites.
  const copyInto = async (dirPath: string, sources: string[]) => {
    setErr(null);
    try {
      await copyIntoDir(sources, dirPath);
    } catch (e) {
      // The backend error names paths and OS codes; the log keeps it, the user gets plain wording.
      console.error("copy into folder failed", safeError(e));
      setErr(t("files.copyFailed"));
    }
    await reloadDir(dirPath);
  };

  // Dropping onto a file targets its containing directory, matching how the create actions resolve a target.
  // Remote clients upload the dropped files; on the desktop the files are on this machine and are copied.
  const onDropFiles = (node: FileNodeT, transfer: DataTransfer) => {
    const dir = node.isDir ? node.path : parentPathOf(node.path);
    if (canTransfer) {
      const { files, skippedFolders } = filesFromDrop(transfer);
      if (files.length || skippedFolders) void uploadInto(dir, files, skippedFolders);
      return;
    }
    // The paths must be requested inside the drop handler, while the drop's data still exists.
    void platform.dialog
      .droppedPaths(transfer)
      .then((dropped) => (dropped.length ? copyInto(dir, dropped.map((d) => d.path)) : undefined))
      .catch((e) => {
        console.error("reading dropped paths failed", safeError(e));
        setErr(t("files.copyFailed"));
      });
  };

  // Open the system file picker for a directory; the chosen files arrive in the input's change handler.
  const pickUpload = (dirPath: string) => {
    uploadDirRef.current = dirPath;
    uploadInputRef.current?.click();
  };

  // Open the context menu at the pointer; create in the directory itself or a file's parent.
  const onContext = (e: React.MouseEvent, node: FileNodeT) => {
    e.preventDefault();
    e.stopPropagation();
    setErr(null);
    void refreshFileClipboardStatus();
    if (root && sameFilePath(node.path, root.path)) {
      setSelPaths([]);
      anchorPath.current = null;
      setFocusPath(node.path);
    } else if (!isPathInList(selPaths, node.path)) {
      setSelPaths([node.path]);
      anchorPath.current = node.path;
      setFocusPath(node.path);
    } else {
      setFocusPath(node.path);
    }
    setMenu({ x: e.clientX, y: e.clientY, node });
  };

  // The header + is equivalent to opening the root context menu beneath the button.
  const onPlus = (e: React.MouseEvent) => {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    setErr(null);
    void refreshFileClipboardStatus();
    setSelPaths([]);
    anchorPath.current = null;
    if (root) setMenu({ x: r.left, y: r.bottom + 4, node: root });
  };

  // Display path relative to the root for Copy Relative Path across POSIX and Windows roots.
  const relativeTo = (p: string) => {
    if (!root) return p;
    if (sameFilePath(p, root.path)) return root.name;
    const isWin = detectFlavor(root.path) === "win";
    const sep = isWin ? "\\" : "/";
    const normRoot = (isWin ? root.path.replace(/\//g, "\\") : root.path).replace(/[\\/]+$/, "");
    const normPath = isWin ? p.replace(/\//g, "\\") : p;
    const cmpRoot = isWin ? normRoot.toLowerCase() : normRoot;
    const cmpPath = isWin ? normPath.toLowerCase() : normPath;
    if (cmpPath.startsWith(cmpRoot + sep)) return normPath.slice(normRoot.length + 1);
    return p;
  };

  // Open a scratch terminal at a directory or a file's parent by passing cwd to newScratchTab.
  const openInTerminal = (node: FileNodeT) => {
    const dir = node.isDir ? node.path : parentPathOf(node.path);
    const nm = baseName(dir, detectFlavor(dir)) || dir;
    useTermStore.getState().newScratchTab({ cwd: dir, name: nm });
  };

  // Scroll the keyboard-focused row into view.
  useEffect(() => {
    if (!focusPath) return;
    treeRef.current?.querySelector(".kbd-focus")?.scrollIntoView?.({ block: "nearest" });
  }, [focusPath]);

  // Focus a row and select it when it is not the workspace root.
  const focusRow = (n: FileNodeT) => {
    setFocusPath(n.path);
    if (root && sameFilePath(n.path, root.path)) return;
    setSelPaths([n.path]);
    anchorPath.current = n.path;
  };

  const displayRoot = useMemo(() => {
    if (!root) return null;
    if (!q) return root;
    if (
      searchState &&
      searchState.rootPath === root.path &&
      searchState.showHidden === showHidden &&
      (searchState.query === q || (q.startsWith(searchState.query) && !searchState.truncated))
    ) {
      return searchState.root;
    }
    return root;
  }, [root, q, searchState, showHidden]);

  const openFilteredPaths = useMemo(
    () => (displayRoot && q ? openFilterDescendants(displayRoot, displayRoot.path, showHidden, q, filterCollapsed) : []),
    [displayRoot, q, showHidden, filterCollapsed],
  );

  const isSameOrDescendant = (candidate: string, ancestor: string) => {
    if (sameFilePath(candidate, ancestor)) return true;
    const isWin = detectFlavor(ancestor) === "win";
    const normA = ancestor.replace(/[\\/]+$/, "").replace(/\\/g, "/");
    const normC = candidate.replace(/\\/g, "/");
    const prefix = normA + "/";
    return isWin ? normC.toLowerCase().startsWith(prefix.toLowerCase()) : normC.startsWith(prefix);
  };

  // Resolve target paths for clipboard/copy-path/delete actions (right-clicked multi-selection, single node, or active selection).
  const selectedActionPaths = (fallbackNode?: FileNodeT): string[] => {
    if (fallbackNode) {
      if (root && sameFilePath(fallbackNode.path, root.path)) return [];
      if (isPathInList(selPaths, fallbackNode.path)) {
        return selPaths.filter((p) => !(root && sameFilePath(p, root.path)));
      }
      return [fallbackNode.path];
    }
    const nonRoot = selPaths.filter((p) => !(root && sameFilePath(p, root.path)));
    if (nonRoot.length) return nonRoot;
    if (focusPath && !(root && sameFilePath(focusPath, root.path))) return [focusPath];
    return [];
  };

  // Resolve the paste destination directory for keyboard/DOM paste based on current selection or focus.
  const keyboardPasteDir = (): string | null => {
    const activeTree = displayRoot ?? root;
    const activePath = selPaths[selPaths.length - 1] ?? focusPath;
    const node = activePath && activeTree ? findNodeByPath(activeTree, activePath) : null;
    if (node) return targetPasteDir(node);
    return root?.path ?? null;
  };

  const doCopy = async (rawPaths: string[]) => {
    const paths = normalizeSourcePaths(rawPaths.filter((p) => !(root && sameFilePath(p, root.path))));
    if (!paths.length) return;
    setErr(null);
    try {
      await copyFilesToClipboard(paths);
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    }
  };

  const doCut = async (rawPaths: string[]) => {
    const paths = normalizeSourcePaths(rawPaths.filter((p) => !(root && sameFilePath(p, root.path))));
    if (!paths.length) return;
    setErr(null);
    try {
      await cutFilesToClipboard(paths);
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    }
  };

  const executePaste = async ({
    dirPath,
    sources,
    mode,
    sequence,
    resolutions,
  }: {
    dirPath: string;
    sources: string[];
    mode: FileClipboardMode;
    sequence?: number;
    resolutions: Record<string, PasteConflictResolution>;
  }) => {
    if (pastingRef.current) return;
    pastingRef.current = true;
    setErr(null);
    try {
      const result = await pasteFiles(sources, dirPath, mode, resolutions);
      if (mode === "cut") {
        await finishCutPaste(sequence);
        const movedSources = sources.filter(
          (src) => !isPathInList(result.skipped, src) && !sameFilePath(parentPathOf(src), dirPath),
        );
        if (previewNode && movedSources.some((src) => isSameOrDescendant(previewNode.path, src))) {
          closePreview();
        }
      }
      const landed = [...result.created, ...result.replaced];
      if (landed.length) {
        setSelPaths(landed);
        anchorPath.current = landed[landed.length - 1];
        setFocusPath(landed[landed.length - 1]);
      } else if (mode === "cut") {
        setSelPaths((prev) => prev.filter((p) => !sources.some((src) => isSameOrDescendant(p, src))));
      }
      const affectedDirs = [dirPath, ...(mode === "cut" ? sources.map(parentPathOf) : [])];
      await reloadDirs(affectedDirs, [dirPath]);
    } catch (e) {
      console.error("paste files failed", safeError(e));
      setErr(e instanceof Error && e.message ? e.message : t("files.pasteFailed"));
    } finally {
      pastingRef.current = false;
    }
  };

  const doPaste = async (dirPath: string) => {
    if (pastingRef.current || conflictPrompt) return;
    pastingRef.current = true;
    setErr(null);
    let readyToExecute: {
      dirPath: string;
      sources: string[];
      mode: FileClipboardMode;
      sequence?: number;
    } | null = null;
    try {
      const clip = await readFilesForPaste();
      if (!clip || !clip.paths.length) return;
      const checks = await pasteFilesCheck(clip.paths, dirPath, clip.mode);
      const conflicts = checks.filter((c) => c.status === "conflict");
      if (conflicts.length > 0) {
        setConflictPrompt({
          dirPath,
          sources: clip.paths,
          mode: clip.mode,
          sequence: clip.sequence,
          conflicts,
          index: 0,
          resolutions: {},
        });
        return;
      }
      readyToExecute = {
        dirPath,
        sources: clip.paths,
        mode: clip.mode,
        sequence: clip.sequence,
      };
    } catch (e) {
      console.error("paste check failed", safeError(e));
      setErr(e instanceof Error && e.message ? e.message : t("files.pasteFailed"));
    } finally {
      pastingRef.current = false;
    }
    if (readyToExecute) {
      await executePaste({ ...readyToExecute, resolutions: {} });
    }
  };

  const resolveConflict = (resolution: PasteConflictResolution, applyAll: boolean) => {
    if (!conflictPrompt) return;
    const { conflicts, index, resolutions, dirPath, sources, mode, sequence } = conflictPrompt;
    const nextResolutions = { ...resolutions };
    if (applyAll) {
      for (let i = index; i < conflicts.length; i++) {
        nextResolutions[conflicts[i].source] = resolution;
      }
      setConflictPrompt(null);
      void executePaste({ dirPath, sources, mode, sequence, resolutions: nextResolutions });
      return;
    }
    nextResolutions[conflicts[index].source] = resolution;
    const nextIndex = index + 1;
    if (nextIndex >= conflicts.length) {
      setConflictPrompt(null);
      void executePaste({ dirPath, sources, mode, sequence, resolutions: nextResolutions });
      return;
    }
    setConflictPrompt({
      ...conflictPrompt,
      index: nextIndex,
      resolutions: nextResolutions,
    });
  };

  // Tree keys: clipboard shortcuts, Escape to cancel cut/multi-select, arrows navigate/extend/expand, and Enter opens.
  const onTreeKeyDown = (e: React.KeyboardEvent) => {
    const activeRoot = displayRoot ?? root;
    if (!activeRoot || conflictPrompt || del || dialog) return;
    const target = e.target as HTMLElement | null;
    if (target?.closest("input, textarea, [contenteditable]")) return;

    if (e.key === "Escape") {
      if (clipboard.cutPaths.length > 0) {
        e.preventDefault();
        e.stopPropagation();
        void clearCutClipboard();
        return;
      }
      if (selPaths.length > 1) {
        const keep = anchorPath.current && isPathInList(selPaths, anchorPath.current)
          ? anchorPath.current
          : selPaths[selPaths.length - 1];
        setSelPaths(keep ? [keep] : []);
        e.preventDefault();
        e.stopPropagation();
        return;
      }
    }

    if (hasMod(e) && !e.shiftKey && !e.altKey) {
      const key = e.key.toLowerCase();
      if (key === "c") {
        const paths = selectedActionPaths();
        if (!paths.length) return;
        e.preventDefault();
        e.stopPropagation();
        lastKeyClipboardRef.current = { op: "copy", at: Date.now() };
        void doCopy(paths);
        return;
      }
      if (key === "x") {
        const paths = selectedActionPaths();
        if (!paths.length) return;
        e.preventDefault();
        e.stopPropagation();
        lastKeyClipboardRef.current = { op: "cut", at: Date.now() };
        void doCut(paths);
        return;
      }
      if (key === "v") {
        const dir = keyboardPasteDir();
        if (!dir) return;
        e.preventDefault();
        e.stopPropagation();
        lastKeyClipboardRef.current = { op: "paste", at: Date.now() };
        void doPaste(dir);
        return;
      }
    }

    if (!["ArrowDown", "ArrowUp", "ArrowRight", "ArrowLeft", "Enter"].includes(e.key)) return;
    const rows = flattenVisibleNodes(activeRoot, activeRoot.path, showHidden, q, filterCollapsed);
    if (!rows.length) return;
    e.preventDefault();
    e.stopPropagation();
    const idx = rows.findIndex((n) => focusPath ? sameFilePath(n.path, focusPath) : false);
    const cur = idx >= 0 ? rows[idx] : null;
    const curKids = cur?.isDir ? visibleChildren(cur, activeRoot.path, showHidden, q) : [];
    const curOpen = cur ? isNodeOpen(cur, activeRoot.path, q, filterCollapsed, curKids.length) : false;
    switch (e.key) {
      case "ArrowDown": {
        const next = rows[idx < 0 ? 0 : Math.min(rows.length - 1, idx + 1)];
        if (e.shiftKey && !sameFilePath(next.path, activeRoot.path)) {
          const order = visibleSelectablePaths();
          const anchor = anchorPath.current && isPathInList(order, anchorPath.current)
            ? anchorPath.current
            : next.path;
          if (!anchorPath.current || !isPathInList(order, anchorPath.current)) {
            anchorPath.current = next.path;
          }
          setFocusPath(next.path);
          setSelPaths(rangeBetweenPaths(order, anchor, next.path));
        } else {
          focusRow(next);
        }
        break;
      }
      case "ArrowUp": {
        const next = rows[idx <= 0 ? 0 : idx - 1];
        if (e.shiftKey && !sameFilePath(next.path, activeRoot.path)) {
          const order = visibleSelectablePaths();
          const anchor = anchorPath.current && isPathInList(order, anchorPath.current)
            ? anchorPath.current
            : next.path;
          if (!anchorPath.current || !isPathInList(order, anchorPath.current)) {
            anchorPath.current = next.path;
          }
          setFocusPath(next.path);
          setSelPaths(rangeBetweenPaths(order, anchor, next.path));
        } else {
          focusRow(next);
        }
        break;
      }
      case "ArrowRight":
        if (cur?.isDir && !curOpen && (!q || canExpandInFilter(cur, curKids.length))) void onDir(cur);
        else if (cur?.isDir && curOpen && idx + 1 < rows.length) focusRow(rows[idx + 1]);
        break;
      case "ArrowLeft":
        if (cur?.isDir && curOpen && !sameFilePath(cur.path, activeRoot.path)) void onDir(cur);
        else if (cur) {
          const parent = rows.find((n) => sameFilePath(n.path, parentPathOf(cur.path)));
          if (parent) focusRow(parent);
        }
        break;
      case "Enter":
        if (cur?.isDir) void onDir(cur);
        else if (cur) onOpen(cur);
        break;
    }
  };

  // Build node-specific or multi-selection context actions for clipboard, create, open, copy path, rename, and delete.
  const menuItems = (node: FileNodeT): MenuItem[] => {
    const activeTree = displayRoot ?? root!;
    const dir = node.isDir ? node : findNodeByPath(activeTree, parentPathOf(node.path)) ?? activeTree;
    const isRoot = !!root && sameFilePath(node.path, root.path);
    const parent = findNodeByPath(activeTree, parentPathOf(node.path)) ?? activeTree;
    const targets = selectedActionPaths(node);
    const many = targets.length > 1;
    const modPrefix = env.isMac ? "⌘" : "Ctrl+";
    const clipboardItems: MenuItem[] = [
      {
        label: t("common.cut"),
        icon: <Icons.scissors size={14} />,
        shortcut: `${modPrefix}X`,
        disabled: isRoot || !targets.length,
        onClick: () => void doCut(targets),
      },
      {
        label: t("common.copy"),
        icon: <Icons.copy size={14} />,
        shortcut: `${modPrefix}C`,
        disabled: isRoot || !targets.length,
        onClick: () => void doCopy(targets),
      },
      {
        label: t("common.paste"),
        icon: <Icons.clipboard size={14} />,
        shortcut: `${modPrefix}V`,
        disabled: !clipboard.canPaste,
        onClick: () => void doPaste(dir.path),
      },
    ];
    const copyPathItems: MenuItem[] = [
      {
        label: t("files.copyPath"),
        icon: <Icons.copy size={14} />,
        onClick: () => {
          notifyTextCopied();
          void copyText(targets.length > 1 ? targets.join("\n") : node.path);
        },
      },
      {
        label: t("files.copyRelPath"),
        icon: <Icons.copy size={14} />,
        onClick: () => {
          notifyTextCopied();
          void copyText(targets.length > 1 ? targets.map(relativeTo).join("\n") : relativeTo(node.path));
        },
      },
    ];
    const deleteItem: MenuItem = {
      label: t("common.delete"),
      icon: <Icons.trash size={14} />,
      danger: true,
      disabled: isRoot || !targets.length,
      onClick: () => {
        setErr(null);
        const deleteTargets = normalizeSourcePaths(targets);
        if (!deleteTargets.length) return;
        const single = deleteTargets.length === 1 ? (findNodeByPath(activeTree, deleteTargets[0]) ?? node) : null;
        setDel({
          paths: deleteTargets,
          label: single ? t("files.deleteConfirm", single.name) : t("files.deleteManyConfirm", deleteTargets.length),
        });
      },
    };
    if (many) {
      return [
        ...clipboardItems,
        { separator: true, label: "" },
        ...copyPathItems,
        { separator: true, label: "" },
        deleteItem,
      ];
    }
    return [
      {
        label: t("files.newFile"),
        icon: <Icons.filePlus size={14} />,
        onClick: () => {
          setErr(null);
          setDialog({ mode: "newFile", dir });
        },
      },
      {
        label: t("files.newFolder"),
        icon: <Icons.folder size={14} />,
        onClick: () => {
          setErr(null);
          setDialog({ mode: "newDir", dir });
        },
      },
      { separator: true, label: "" },
      // Group editor (files only), terminal, and file-manager open/reveal actions.
      ...(!node.isDir
        ? ([
            {
              label: t("panel.preview"),
              icon: <Icons.eye size={14} />,
              onClick: () => void doPreview(node),
            },
            {
              label: t("panel.openInEditor"),
              icon: <Icons.code size={14} />,
              onClick: () => onOpen(node),
            },
          ] as MenuItem[])
        : []),
      // Transfer actions, shown only for remote access where the server is a different machine.
      ...(canTransfer
        ? ([
            ...(node.isDir
              ? []
              : [
                  {
                    label: t("transfer.download"),
                    icon: <Icons.download size={14} />,
                    onClick: () => {
                      setErr(null);
                      startDownload(node.path).catch((e) => setErr(e instanceof Error ? e.message : String(e)));
                    },
                  },
                ]),
            {
              label: t("transfer.upload"),
              icon: <Icons.upload size={14} />,
              onClick: () => pickUpload(dir.path),
            },
            { separator: true, label: "" },
          ] as MenuItem[])
        : []),
      {
        label: t("files.openInTerminal"),
        icon: <Icons.terminal size={14} />,
        onClick: () => openInTerminal(node),
      },
      {
        label: t("files.revealInFinder"),
        icon: <Icons.reveal size={14} />,
        onClick: () => void revealPath(node.path),
      },
      {
        label: t("common.refresh"),
        icon: <Icons.restart size={14} />,
        onClick: () => void reloadDir(dir.path),
      },
      { separator: true, label: "" },
      ...clipboardItems,
      { separator: true, label: "" },
      ...copyPathItems,
      { separator: true, label: "" },
      {
        label: t("common.rename"),
        icon: <Icons.rename size={14} />,
        disabled: isRoot,
        onClick: () => {
          setErr(null);
          setDialog({ mode: "rename", dir: parent, node });
        },
      },
      deleteItem,
    ];
  };

  // Submit a create or rename operation.
  const submitDialog = async (rawName: string) => {
    const name = rawName.trim();
    if (!dialog || !name) return;
    const d = dialog;
    setDialog(null);
    try {
      if (d.mode === "rename" && d.node) {
        const to = `${parentPathOf(d.node.path)}/${name}`;
        if (sameFilePath(to, d.node.path)) return; // The name did not change.
        await renamePath(d.node.path, to);
        setSelPaths((prev) => prev.filter((p) => !sameFilePath(p, d.node!.path)));
        if (previewNode && sameFilePath(previewNode.path, d.node.path)) closePreview();
        await reloadDir(d.dir.path);
      } else {
        const path = `${d.dir.path}/${name}`;
        if (d.mode === "newFile") {
          await createFile(path);
          await reloadDir(d.dir.path);
          useTermStore.getState().openDocTab(path); // Open a new file immediately for editing.
        } else {
          await createDir(path);
          await reloadDir(d.dir.path);
        }
      }
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    }
  };

  // Execute single or multi-item deletion after confirmation.
  const doDelete = async () => {
    if (!del) return;
    const d = del;
    setDel(null);
    const affectedParents: string[] = [];
    const deleted: string[] = [];
    try {
      for (const path of d.paths) {
        await deletePath(path);
        deleted.push(path);
        affectedParents.push(parentPathOf(path));
      }
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      if (deleted.length) {
        setSelPaths((prev) => prev.filter((p) => !deleted.some((target) => isSameOrDescendant(p, target))));
        if (previewNode && deleted.some((target) => isSameOrDescendant(previewNode.path, target))) {
          closePreview();
        }
        await reloadDirs(affectedParents);
      }
    }
  };

  if (!root) {
    return (
      <div className="insp-section" style={{ color: "var(--text-faint)" }}>
        {t("panel.noSession")}
      </div>
    );
  }

  const activeRoot = displayRoot ?? root;
  const lastSelNode = selPaths.length ? findNodeByPath(activeRoot, selPaths[selPaths.length - 1]) : null;
  const showPreview = !!previewNode;
  const previewBadge = previewNode
    ? (findNodeByPath(activeRoot, previewNode.path)?.badge ?? findNodeByPath(root, previewNode.path)?.badge)
    : null;
  const visibleError = err ?? refreshErr;
  const canCollapse = q ? openFilteredPaths.length > 0 : hasOpenDescendant(root);
  const onCollapseAll = () => {
    if (q) {
      if (!openFilteredPaths.length) return;
      setFilterCollapsed((prev) => new Set([...prev, ...openFilteredPaths]));
      return;
    }
    setRoot((r) => (r ? collapseAllBelow(r) : r));
  };
  const showTruncated =
    !!q &&
    searchState?.rootPath === root.path &&
    searchState.showHidden === showHidden &&
    searchState.query === q &&
    searchState.truncated;

  return (
    <div ref={wrapRef} style={{ display: "flex", flexDirection: "column", minHeight: 0, flex: 1 }}>
      <div className="files-head">
        <span className="files-path" title={root.path}>{root.name}</span>
        {/* Disabled rather than hidden when nothing is expanded so the button stays discoverable and
            keeps toolbar alignment steady; while filtering, collapses only the active filter view. */}
        <button
          className="files-toggle"
          title={t("files.collapseAll")}
          aria-label={t("files.collapseAll")}
          disabled={!canCollapse}
          onClick={onCollapseAll}
        >
          <Icons.collapse size={13} />
        </button>
        <button
          className={"files-toggle" + (filterOn ? " on" : "")}
          title={t("files.filterPlaceholder")}
          aria-label={t("files.filterPlaceholder")}
          aria-pressed={filterOn}
          onClick={() => setFilterOn((v) => { if (v) setFilter(""); return !v; })}
        >
          <Icons.search size={13} />
        </button>
        <button className="files-toggle" title={t("files.newTooltip")} aria-label={t("files.newTooltip")} onClick={onPlus}>
          <Icons.filePlus size={13} />
        </button>
        {canTransfer && (
          <button
            className="files-toggle"
            title={t("transfer.uploadTooltip")}
            aria-label={t("transfer.uploadTooltip")}
            onClick={() => pickUpload(lastSelNode?.isDir ? lastSelNode.path : root.path)}
          >
            <Icons.upload size={13} />
          </button>
        )}
        <button
          className={"files-toggle" + (showHidden ? " on" : "")}
          title={showHidden ? t("panel.hideHidden") : t("panel.showHidden")}
          aria-label={showHidden ? t("panel.hideHidden") : t("panel.showHidden")}
          aria-pressed={showHidden}
          onClick={() => setShowHidden((v) => !v)}
        >
          {showHidden ? <Icons.eye size={13} /> : <Icons.eyeOff size={13} />}
        </button>
      </div>
      {filterOn && (
        <div className="files-filter">
          {searching ? (
            <span className="spin" style={{ display: "inline-flex" }}>
              <Icons.restart size={12} />
            </span>
          ) : (
            <Icons.search size={12} />
          )}
          <input
            autoFocus
            value={filter}
            placeholder={t("files.filterPlaceholder")}
            onChange={(e) => setFilter(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                setFilter("");
                setFilterOn(false);
              }
            }}
          />
          {filter && (
            <button
              className="files-filter-clear"
              title={t("common.close")}
              aria-label={t("common.close")}
              onClick={() => setFilter("")}
            >
              <Icons.x size={12} />
            </button>
          )}
        </div>
      )}
      {visibleError && (
        <div className="files-err" title={visibleError} onClick={() => { setErr(null); setRefreshErr(null); }}>
          {visibleError}
        </div>
      )}
      <div
        ref={treeRef}
        tabIndex={0}
        className="files-tree"
        style={{ flex: "1 1 0", overflowY: "auto", minHeight: 0, outline: "none" }}
        onKeyDown={onTreeKeyDown}
        onCopy={(e) => {
          const target = e.target as HTMLElement | null;
          if (target?.closest("input, textarea, [contenteditable]")) return;
          if (lastKeyClipboardRef.current?.op === "copy" && Date.now() - lastKeyClipboardRef.current.at < 120) {
            e.preventDefault();
            e.stopPropagation();
            return;
          }
          const paths = selectedActionPaths();
          if (!paths.length) return;
          e.preventDefault();
          e.stopPropagation();
          void doCopy(paths);
        }}
        onCut={(e) => {
          const target = e.target as HTMLElement | null;
          if (target?.closest("input, textarea, [contenteditable]")) return;
          if (lastKeyClipboardRef.current?.op === "cut" && Date.now() - lastKeyClipboardRef.current.at < 120) {
            e.preventDefault();
            e.stopPropagation();
            return;
          }
          const paths = selectedActionPaths();
          if (!paths.length) return;
          e.preventDefault();
          e.stopPropagation();
          void doCut(paths);
        }}
        onPaste={(e) => {
          const target = e.target as HTMLElement | null;
          if (target?.closest("input, textarea, [contenteditable]")) return;
          if (lastKeyClipboardRef.current?.op === "paste" && Date.now() - lastKeyClipboardRef.current.at < 120) {
            e.preventDefault();
            e.stopPropagation();
            return;
          }
          const dir = keyboardPasteDir();
          if (!dir) return;
          e.preventDefault();
          e.stopPropagation();
          void doPaste(dir);
        }}
        // Rows take drops themselves; a drop on the empty space below them goes to the root directory.
        onDragOver={(e) => {
          if (!canDropFiles || !e.dataTransfer.types.includes("Files")) return;
          e.preventDefault();
          e.dataTransfer.dropEffect = "copy";
        }}
        onDrop={(e) => {
          if (!canDropFiles || !root || !e.dataTransfer.types.includes("Files")) return;
          e.preventDefault();
          onDropFiles(root, e.dataTransfer);
        }}
        onContextMenu={(e) => {
          // Right-clicking empty tree space creates or pastes in the root directory.
          e.preventDefault();
          setErr(null);
          void refreshFileClipboardStatus();
          setSelPaths([]);
          anchorPath.current = null;
          if (root) setMenu({ x: e.clientX, y: e.clientY, node: root });
        }}
      >
        <FileRow
          node={activeRoot}
          rootPath={activeRoot.path}
          depth={0}
          selPaths={selPaths}
          cutPaths={clipboard.cutPaths}
          focusPath={focusPath}
          showHidden={showHidden}
          filter={q}
          filterCollapsed={filterCollapsed}
          onFile={onFile}
          onOpen={onOpen}
          onDir={onDir}
          onContext={onContext}
          onDropFiles={canDropFiles ? onDropFiles : null}
        />
        {showTruncated && (
          <div className="files-filter-truncated">{t("files.filterTruncated")}</div>
        )}
      </div>
      {canTransfer && (
        <input
          ref={uploadInputRef}
          type="file"
          multiple
          style={{ display: "none" }}
          onChange={(e) => {
            const dir = uploadDirRef.current;
            const files = Array.from(e.target.files ?? []);
            // Reset the value so choosing the same file twice in a row still fires change.
            e.target.value = "";
            if (dir && files.length) void uploadInto(dir, files);
          }}
        />
      )}
      {transfers.length > 0 && <TransferQueue items={transfers} />}
      {showPreview && previewNode && (
        <>
          {/* Vertically draggable divider between the file tree and the preview pane. */}
          <div
            onMouseDown={startPreviewResize}
            title={t("splitter.dragToResize")}
            style={{ flex: "0 0 5px", cursor: "row-resize", position: "relative", zIndex: 5 }}
          >
            <div
              style={{ position: "absolute", left: 0, right: 0, top: 2, height: 1, background: "var(--border)" }}
            />
          </div>
          <div
            className="preview"
            style={{ flex: `0 0 ${previewH}px`, minHeight: 0, display: "flex", flexDirection: "column" }}
          >
            <div className="preview-head">
              {renderFileIcon(previewNode.name, false)}
              {previewNode.name}
              {previewBadge && <span className={"gb gb-" + previewBadge}>{previewBadge}</span>}
              <button
                title={t("panel.openInEditorTooltip")}
                style={{
                  marginLeft: "auto",
                  padding: "1px 8px",
                  border: "1px solid var(--border-strong)",
                  borderRadius: "var(--r-sm)",
                  fontSize: 11,
                  color: "var(--text)",
                  flex: "none",
                }}
                onClick={() => useTermStore.getState().openDocTab(previewNode.path)}
              >
                {t("panel.openInEditor")}
              </button>
              <button
                title={t("common.close")}
                aria-label={t("common.close")}
                onClick={closePreview}
                style={{
                  flex: "none",
                  display: "grid",
                  placeItems: "center",
                  width: 20,
                  height: 20,
                  borderRadius: "var(--r-sm)",
                  border: "none",
                  background: "transparent",
                  color: "var(--text-faint)",
                  cursor: "pointer",
                }}
              >
                <Icons.x size={13} />
              </button>
            </div>
            <pre style={{ flex: 1, overflow: "auto" }}>
              {preview == null ? (
                t("common.loading")
              ) : preview.binary ? (
                t("panel.binary")
              ) : (
                <>
                  {highlight(preview.content)}
                  {preview.truncated ? t("panel.truncated") : null}
                </>
              )}
            </pre>
          </div>
        </>
      )}
      {menu && (
        <ContextMenu x={menu.x} y={menu.y} items={menuItems(menu.node)} onClose={() => setMenu(null)} />
      )}
      {dialog && (
        <FormModal
          title={
            dialog.mode === "rename"
              ? t("common.rename")
              : dialog.mode === "newDir"
                ? t("files.newFolder")
                : t("files.newFile")
          }
          fields={[{ key: "name", label: t("files.nameLabel"), required: true, autoFocus: true }]}
          initial={dialog.mode === "rename" ? { name: dialog.node?.name ?? "" } : undefined}
          submitLabel={dialog.mode === "rename" ? t("common.rename") : t("common.create")}
          onSubmit={(v) => void submitDialog(v.name)}
          onCancel={() => setDialog(null)}
        />
      )}
      {del && (
        <ConfirmModal
          body={del.label}
          confirmLabel={t("common.delete")}
          onConfirm={() => void doDelete()}
          onCancel={() => setDel(null)}
        />
      )}
      {conflictPrompt && (
        <PasteConflictModal
          conflicts={conflictPrompt.conflicts}
          index={conflictPrompt.index}
          onResolve={resolveConflict}
          onCancel={() => setConflictPrompt(null)}
        />
      )}
    </div>
  );
}

/** Time left as m:ss or h:mm:ss, which needs no translation and reads the same as any download manager. */
function formatDuration(seconds: number): string {
  const s = Math.max(0, Math.round(seconds));
  const pad = (n: number) => String(n).padStart(2, "0");
  if (s < 3600) return `${Math.floor(s / 60)}:${pad(s % 60)}`;
  return `${Math.floor(s / 3600)}:${pad(Math.floor((s % 3600) / 60))}:${pad(s % 60)}`;
}

/** Human-readable byte size for transfer progress, at the coarsest unit that still reads precisely. */
function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

/**
 * Transfer queue strip below the tree.
 *
 * It lives here rather than inside the tree because transfers are panel-wide, and it reads from the module-level
 * store so an upload survives switching tabs.
 */
function TransferQueue({ items }: { items: Transfer[] }) {
  const t = useT();
  const anyFinished = items.some((x) => x.state !== "active");
  return (
    <div className="transfer-queue">
      <div className="transfer-head">
        <span>{t("transfer.title")}</span>
        {anyFinished && (
          <button className="transfer-clear" onClick={clearFinishedTransfers}>
            {t("transfer.clear")}
          </button>
        )}
      </div>
      {items.map((x) => {
        const pct = x.total > 0 ? Math.min(100, Math.round((x.transferred / x.total) * 100)) : 0;
        const size = x.total > 0 ? `${formatBytes(x.transferred)} / ${formatBytes(x.total)}` : formatBytes(x.transferred);
        // Rate and time left only appear once they are measured, so the line never shows a placeholder value.
        const rate = x.bytesPerSec ? ` · ${formatBytes(x.bytesPerSec)}/s` : "";
        const eta = x.etaSec != null && x.etaSec > 0 ? ` · ${formatDuration(x.etaSec)}` : "";
        const download = x.direction === "download";
        // Only a download the webview saved itself (account remote windows) has no progress and cannot be
        // stopped; the desktop host's downloads report both, like uploads.
        const measured = !download || x.localId != null;
        const running = measured && (x.state === "active" || x.state === "stalled");
        const detail =
          x.state === "failed"
            ? x.error || t("transfer.failed")
            : x.state === "cancelled"
              ? t("transfer.cancelled")
              : x.state === "stalled"
                ? `${t("transfer.stalled")} · ${size}`
                : !measured
                  ? t(x.state === "done" ? "transfer.savedToDownloads" : "transfer.downloading")
                  : size + rate + eta;
        return (
          <div key={x.id} className={"transfer-row " + x.state} title={x.savedPath ?? x.path}>
            <span className="transfer-dir">
              {download ? <Icons.download size={12} /> : <Icons.upload size={12} />}
            </span>
            <span className="transfer-name">{x.name}</span>
            <span className="transfer-detail">{detail}</span>
            {running ? (
              <button
                className="transfer-cancel"
                title={t("common.cancel")}
                aria-label={t("common.cancel")}
                onClick={() => cancelTransfer(x.id)}
              >
                <Icons.x size={11} />
              </button>
            ) : (
              <span className="transfer-cancel" />
            )}
            {running && (
              <span className="transfer-bar">
                <span style={{ width: `${pct}%` }} />
              </span>
            )}
          </div>
        );
      })}
    </div>
  );
}

/** File deletion confirmation, separate from sessionMenu to avoid coupling file and session semantics. */
function ConfirmModal({
  body,
  confirmLabel,
  onConfirm,
  onCancel,
}: {
  body: string;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const t = useT();
  return (
    <Backdrop onClose={onCancel}>
      <div
        style={{
          width: 340,
          background: "var(--bg-panel)",
          border: "1px solid var(--border)",
          borderRadius: 10,
          padding: 18,
          boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.6, marginBottom: 18 }}>
          {body}
        </div>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button className="vlx-btn" onClick={onCancel}>
            {t("common.cancel")}
          </button>
          <button
            className="vlx-btn vlx-btn-primary"
            style={{ background: "var(--status-error)", borderColor: "var(--status-error)" }}
            onClick={onConfirm}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </Backdrop>
  );
}

/** Prompt shown before executing a paste when one or more items already exist at the destination. */
function PasteConflictModal({
  conflicts,
  index,
  onResolve,
  onCancel,
}: {
  conflicts: PasteCheckItem[];
  index: number;
  onResolve: (resolution: PasteConflictResolution, applyAll: boolean) => void;
  onCancel: () => void;
}) {
  const t = useT();
  const [applyAll, setApplyAll] = useState(false);
  const current = conflicts[index];
  if (!current) return null;
  const remaining = conflicts.length - index;

  return (
    <Backdrop onClose={onCancel}>
      <div
        style={{
          width: 420,
          maxWidth: "calc(100vw - 32px)",
          background: "var(--bg-panel)",
          border: "1px solid var(--border)",
          borderRadius: 10,
          padding: 18,
          boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, marginBottom: 10 }}>
          <div style={{ fontSize: 14, fontWeight: 600, color: "var(--text-primary)" }}>
            {t("files.pasteConflictTitle")}
          </div>
          {conflicts.length > 1 && (
            <span style={{ fontSize: 12, color: "var(--text-faint)" }}>
              {t("files.pasteConflictProgress", index + 1, conflicts.length)}
            </span>
          )}
        </div>
        <div style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.6, marginBottom: remaining > 1 ? 12 : 18 }}>
          {t("files.pasteConflictBody", current.name)}
        </div>
        {remaining > 1 && (
          <label
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              fontSize: 12,
              color: "var(--text-secondary)",
              marginBottom: 16,
              cursor: "pointer",
              userSelect: "none",
            }}
          >
            <input
              type="checkbox"
              checked={applyAll}
              onChange={(e) => setApplyAll(e.target.checked)}
            />
            <span>{t("files.pasteConflictApplyAll")}</span>
          </label>
        )}
        <div style={{ display: "flex", justifyContent: "flex-end", flexWrap: "wrap", gap: 8 }}>
          <button className="vlx-btn" onClick={onCancel}>
            {t("common.cancel")}
          </button>
          <button className="vlx-btn" onClick={() => onResolve("skip", applyAll)}>
            {t("files.pasteSkip")}
          </button>
          <button className="vlx-btn" autoFocus onClick={() => onResolve("keepBoth", applyAll)}>
            {t("files.pasteKeepBoth")}
          </button>
          <button className="vlx-btn vlx-btn-primary" onClick={() => onResolve("replace", applyAll)}>
            {t("files.pasteReplace")}
          </button>
        </div>
      </div>
    </Backdrop>
  );
}
