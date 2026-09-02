//! Vlinx-style sidebar tree for projects, nested groups, sessions, and child sessions. Supports search filtering,
//! drag-and-drop, additive/range selection, context menus, inline rename, and hover-to-create controls.

import { useVirtualizer } from "@tanstack/react-virtual";
import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import Icons from "../../components/Icons";
import { StatusIndicator } from "../../components/StatusIndicator";
import { useT } from "../../i18n";
import {
  type SelNode,
  type SidebarTreeView,
  useTermStore,
} from "../../store/termStore";
import {
  effectiveStatus,
  isVirtualProject,
  type Group,
  type NodeKind,
  type Project,
  type Session,
} from "../../types";
import { MARK_LABEL_KEYS, type NodeMark, normalizeMark } from "../../marks";
import { SessionKindIcon } from "../sessionViewers/sessionMeta";
import { DEFAULT_BINDINGS, formatCombo } from "../../hooks/shortcutRegistry";
import { useGitBranch } from "../../hooks/useGitBranch";

/** WKWebView inserts control characters such as U+001C through beforeinput when Left/Right is pressed past an
 *  input boundary. They render as boxes and are unrelated to IME; Chromium is unaffected. Strip C0/C1 and DEL
 *  so rename fields accept only normal printable text. */
const CTRL_CHARS_RE = /[\u0000-\u001F\u007F-\u009F]/g;
const stripControlChars = (s: string) => s.replace(CTRL_CHARS_RE, "");

/** Attaches a native beforeinput listener that cancels control-character insertion. React's synthetic prevention is
 *  unreliable in WebKit, while onChange sanitization can leave the DOM unchanged when state compares equal. Native
 *  cancellation prevents insertion and caret movement. Returns cleanup for React 19 ref callbacks. */
function useCtrlCharGuard() {
  return useCallback((el: HTMLInputElement | null) => {
    if (!el) return;
    const onBeforeInput = (ev: Event) => {
      const d = (ev as InputEvent).data;
      if (typeof d === "string" && d !== stripControlChars(d)) ev.preventDefault();
    };
    el.addEventListener("beforeinput", onBeforeInput);
    return () => el.removeEventListener("beforeinput", onBeforeInput);
  }, []);
}

/** Reference to a node targeted by a context menu or operation. */
export interface TreeNodeRef {
  kind: NodeKind;
  id: string;
  name: string;
  projectId: string;
  groupId: string | null;
}

interface DragPayload {
  kind: "group" | "session";
  id: string;
  projectId: string;
  /** For multi-session dragging, carries all selected IDs in the same project when at least two are selected.
   *  Drop uses the same moveMany logic as Move Selected To. */
  ids?: string[];
}

/** Tree interaction callbacks supplied by LeftSidebar for menus, inline rename, and hover creation. */
export interface TreeHandlers {
  /** Isolated search/filter projection rendered by this tree instance. */
  view: SidebarTreeView;
  /** Only the primary projection consumes global reveal requests. */
  isPrimary: boolean;
  onContext: (node: TreeNodeRef, x: number, y: number) => void;
  /** Current context-menu target, highlighted temporarily without changing persistent selection. */
  contextId: string | null;
  renamingId: string | null;
  renameVal: string;
  setRenameVal: (v: string) => void;
  commitRename: () => void;
  cancelRename: () => void;
  onAddSession: (node: TreeNodeRef, x: number, y: number) => void;
  onAddGroup: (node: TreeNodeRef) => void;
}

/** One-dimensional virtualized row model flattening the project/group/session recursion. useVirtualizer renders only
 *  visible rows and reuses nodes while scrolling. This is separate from `flat`, which contains only persistent nodes
 *  for Shift-range selection. */
type TreeRow =
  | { kind: "project"; id: string; project: Project; expanded: boolean }
  | { kind: "group"; id: string; group: Group; depth: number; expanded: boolean }
  | {
      kind: "session";
      id: string;
      session: Session;
      depth: number;
      hasKids: boolean;
      expanded: boolean;
    };

function Chevron({ open }: { open: boolean }) {
  return open ? <Icons.chevD size={13} /> : <Icons.chevR size={13} />;
}

/** Emoji marker rendered between the node icon and its name. Unmarked nodes render nothing and consume no width.
 *  Markers outside the known palette still display, using the raw value as the tooltip. */
function NodeMarkBadge({ mark }: { mark?: string | null }) {
  const t = useT();
  const value = normalizeMark(mark);
  if (!value) return null;
  const labelKey = MARK_LABEL_KEYS[value as NodeMark];
  return (
    <span className="node-mark" title={labelKey ? t(labelKey) : value}>
      {value}
    </span>
  );
}

/** Worktree badge on group folder icons. The small .wt-badge overlays the lower-right corner without consuming row
 *  width; its tooltip contains the live Git branch and path, falling back to the directory basename. This is a
 *  component because useGitBranch cannot be called inside renderRow's switch. SessionKindIcon handles session badges. */
function GroupWorktreeBadge({ path }: { path: string }) {
  const t = useT();
  const branch = useGitBranch(path);
  const leaf = path.split("/").filter(Boolean).pop() || path;
  const text = branch || leaf;
  return (
    <span className="wt-badge" title={`${t("spawn.worktreeLabel")}: ${text}\n${path}`}>
      <Icons.branch size={8} sw={2} />
    </span>
  );
}

interface SessionRowProps {
  session: Session;
  /** Owning project root, used to detect worktrees when a session has no cwd. */
  rootPath?: string | null;
  depth: number;
  hasKids: boolean;
  expanded: boolean;
  context: boolean;
  renaming: boolean;
  renameVal?: string;
  draggable: boolean;
  dragHighlight?: React.CSSProperties;
  onRowClick: (node: SelNode, e: React.MouseEvent, isSession: boolean) => void;
  onRowContext: (ref: TreeNodeRef, e: React.MouseEvent) => void;
  onToggle: (id: string) => void;
  onDragStartRow: (payload: DragPayload, e: React.DragEvent) => void;
  onDragOverRow: (id: string, e: React.DragEvent) => void;
  onDragLeaveRow: (id: string) => void;
  onDropRow: (target: Session, e: React.DragEvent) => void;
  onMouseDownRow: (e: React.MouseEvent) => void;
  onAddSession: (ref: TreeNodeRef, e: React.MouseEvent) => void;
  setRenameVal: (v: string) => void;
  commitRename: () => void;
  cancelRename: () => void;
}

/** Session-row states remain independent. `active` means opened in the center pane, while `selected` means membership
 *  in tree selection. An active session excluded from a multi-selection must not appear selected. Exported only for
 *  state-combination regression tests. */
export function sessionRowClassName(
  selected: boolean,
  active: boolean,
  context: boolean,
  unread: boolean,
) {
  return (
    "row session" +
    (selected ? " sel" : "") +
    (active ? " active" : "") +
    (context ? " context" : "") +
    (unread ? " unread" : "")
  );
}

/** Memoized session row. Hundreds may exist, so each subscribes only to its own high-frequency status, active,
 *  unread, and selection values. ProjectTree/LeftSidebar avoid whole runtime collections, letting only the row with
 *  a real boolean-selector change rerender during frequent agent updates. */
const SessionRow = memo(function SessionRow(p: SessionRowProps) {
  const t = useT();
  const ctrlCharGuard = useCtrlCharGuard();
  const s = p.session;
  // Subscribe to the derived status string so Object.is suppresses rerenders when runtime identity alone changes.
  const status = useTermStore((st) => effectiveStatus(st.runtimes[s.id]));
  const active = useTermStore((st) => st.activeSessionId === s.id);
  const unread = useTermStore((st) => s.id in st.notifications);
  const selected = useTermStore((st) => st.selection.some((x) => x.id === s.id));
  const isBrowser = s.kind === "browser";
  const isAgent = s.kind !== "terminal" && !isBrowser;
  const ref: TreeNodeRef = {
    kind: "session",
    id: s.id,
    name: s.name,
    projectId: s.projectId,
    groupId: s.groupId ?? null,
  };
  const dndProps = {
    draggable: p.draggable,
    onDragStart: (e: React.DragEvent) =>
      p.onDragStartRow({ kind: "session", id: s.id, projectId: s.projectId }, e),
    onDragOver: (e: React.DragEvent) => p.onDragOverRow(s.id, e),
    onDragLeave: () => p.onDragLeaveRow(s.id),
    onDrop: (e: React.DragEvent) => p.onDropRow(s, e),
  };
  return (
    <div
      className={sessionRowClassName(selected, active, p.context, unread)}
      style={{ paddingLeft: 6 + p.depth * 13, ...p.dragHighlight }}
      {...dndProps}
      onMouseDown={p.onMouseDownRow}
      onClick={(e) => p.onRowClick({ id: s.id, kind: "session" }, e, true)}
      onContextMenu={(e) => p.onRowContext(ref, e)}
    >
      {p.hasKids ? (
        <span
          className="tw"
          onClick={(e) => {
            e.stopPropagation();
            p.onToggle(s.id);
          }}
        >
          <Chevron open={p.expanded} />
        </span>
      ) : (
        <span className="tw leaf" />
      )}
      <span
        className="ic"
        style={{ color: isAgent ? "var(--text-secondary)" : "var(--text-dim)" }}
        title={s.kind}
      >
        {/* The worktree badge comes with SessionKindIcon: it sits at the bottom right of the session
            kind icon and follows the light/dark theme (see .wt-badge). It takes no inline width and
            does not cover the session name. */}
        <SessionKindIcon session={s} size={isAgent ? 15 : 14} rootPath={p.rootPath} />
      </span>
      {p.renaming ? (
        <input
          ref={ctrlCharGuard}
          className="rename-input"
          autoFocus
          value={p.renameVal ?? ""}
          onClick={(e) => e.stopPropagation()}
          onChange={(e) => p.setRenameVal(stripControlChars(e.target.value))}
          onKeyDown={(e) => {
            if (e.key === "Enter") p.commitRename();
            if (e.key === "Escape") p.cancelRename();
          }}
          onBlur={p.commitRename}
        />
      ) : (
        <>
          <NodeMarkBadge mark={s.mark} />
          <span className="nm">{s.name}</span>
        </>
      )}
      {/* Browser page nodes have no PTY or agent, so a status dot would be meaningless and is omitted. */}
      {!isBrowser && <StatusIndicator status={status} unread={unread} />}
      <span className="meta">
        <span
          className="add"
          title={t("tree.newChildSession")}
          onClick={(e) => {
            e.stopPropagation();
            p.onAddSession(ref, e);
          }}
        >
          <Icons.plus size={12} />
        </span>
      </span>
    </div>
  );
});

export function ProjectTree(h: TreeHandlers) {
  const t = useT();
  const ctrlCharGuard = useCtrlCharGuard();
  const {
    onContext,
    contextId,
    renamingId,
    renameVal,
    setRenameVal,
    commitRename,
    cancelRename,
    onAddSession,
    onAddGroup,
    view,
    isPrimary,
  } = h;

  const projects = useTermStore((s) => s.projects);
  const treeLoaded = useTermStore((s) => s.treeLoaded);
  // Delay the loading row so a fast load stays blank instead of flashing a spinner for one frame.
  const [showLoadingHint, setShowLoadingHint] = useState(false);
  useEffect(() => {
    if (treeLoaded) return;
    const timer = setTimeout(() => setShowLoadingHint(true), 200);
    return () => clearTimeout(timer);
  }, [treeLoaded]);
  const setCreateProjectModalOpen = useTermStore((s) => s.setCreateProjectModalOpen);
  const importProject = useTermStore((s) => s.importProject);
  const setCloneModalOpen = useTermStore((s) => s.setCloneModalOpen);
  // Format the user override or default Open Project shortcut for the empty-state hint.
  const openProjectCombo =
    useTermStore((s) => s.shortcutOverrides.openProject) || DEFAULT_BINDINGS.openProject;
  const groups = useTermStore((s) => s.groups);
  const sessions = useTermStore((s) => s.sessions);
  const ephemeralSessions = useTermStore((s) => s.ephemeralSessions);
  const toggleCollapsed = useTermStore((s) => s.toggleCollapsed);
  const openSession = useTermStore((s) => s.openSession);
  // activeSessionId changes only on user navigation, so subscribing here for reveal behavior does not compromise
  // per-row isolation of high-frequency runtime updates.
  const activeSessionId = useTermStore((s) => s.activeSessionId);
  const revealProjectId = useTermStore((s) => s.revealProjectId);
  const setRevealProject = useTermStore((s) => s.setRevealProject);
  // Subscribe to density/navigation layout so virtualization receives the exact row height after changes.
  const density = useTermStore((s) => s.density);
  const navLayout = useTermStore((s) => s.navLayout);
  const moveNode = useTermStore((s) => s.moveNode);
  const moveMany = useTermStore((s) => s.moveMany);
  const {
    treeFilter,
    statusFilter,
    statusFilterIds,
    markFilter,
  } = view;
  const selection = useTermStore((s) => s.selection);
  const selectionAnchor = useTermStore((s) => s.selectionAnchor);
  const selectSingle = useTermStore((s) => s.selectSingle);
  const toggleSelect = useTermStore((s) => s.toggleSelect);
  const setSelection = useTermStore((s) => s.setSelection);
  const setInspectTarget = useTermStore((s) => s.setInspectTarget);

  type DragZone = "top" | "center" | "bottom";
  interface DragTarget { id: string; zone: DragZone }
  const [dragOver, setDragOver] = useState<DragTarget | null>(null);

  // Virtualized scroll container. Store it in both a ref for useVirtualizer and state so drag autoscroll listeners
  // track mounting and unmounting correctly.
  const parentRef = useRef<HTMLDivElement | null>(null);
  const [scrollEl, setScrollEl] = useState<HTMLDivElement | null>(null);
  const setParent = useCallback((el: HTMLDivElement | null) => {
    parentRef.current = el;
    setScrollEl(el);
  }, []);

  // The primary pane reads and writes the shared collapse state kept in the database. Every split-off pane keeps its
  // own map instead, so expanding a node here never moves the other panes. Nodes created after the split have no
  // entry yet and fall back to the shared value.
  const setViewCollapsed = useTermStore((s) => s.setSidebarTreeViewCollapsed);
  const collapsedOverrides = useMemo(
    () => (isPrimary ? null : (view.collapsedOverrides ?? {})),
    [isPrimary, view.collapsedOverrides],
  );
  const nodeCollapsed = (id: string, shared: boolean | undefined) =>
    collapsedOverrides && id in collapsedOverrides ? collapsedOverrides[id] : !!shared;
  const toggleNodeCollapsed = (kind: NodeKind, id: string, shared: boolean | undefined) => {
    if (!collapsedOverrides) {
      void toggleCollapsed(kind, id);
      return;
    }
    setViewCollapsed(view.id, id, !nodeCollapsed(id, shared));
  };

  const filter = treeFilter.trim().toLowerCase();
  const statusFiltering = statusFilter !== null;
  const markFiltering = markFilter !== null;
  const filtering = filter.length > 0 || statusFiltering || markFiltering;

  // Marker filtering compares live values rather than a snapshot: a marker changes only when the user picks one,
  // so no row can disappear while it is being clicked.
  const markMatch = (mark?: string | null) => normalizeMark(mark) === markFilter;
  // A marked group/project stands for everything inside it, so a marker hit propagates down its subtree exactly
  // like a name hit.
  const markHit = (mark?: string | null) => markFiltering && markMatch(mark);

  // Status filtering uses the snapshot captured when enabled. Subsequent status changes update dots but do not add
  // or remove rows until the filter changes, preventing an Awaiting item from disappearing as soon as it is opened.
  const statusMatch = (s: Session) => !!statusFilterIds && s.id in statusFilterIds;

  // Status and marker are alternatives, not an intersection: with both active a session qualifies on either one.
  const attributeMatch = (s: Session) =>
    (!statusFiltering && !markFiltering) ||
    (statusFiltering && statusMatch(s)) ||
    (markFiltering && markMatch(s.mark));

  const groupsById = useMemo(
    () => new Map(groups.map((g) => [g.id, g])),
    [groups],
  );
  const sessionsById = useMemo(
    () => new Map(sessions.map((s) => [s.id, s])),
    [sessions],
  );
  const selectedIds = useMemo(
    () => new Set(selection.map((s) => s.id)),
    [selection],
  );

  // During filtering, keep matching sessions and their ancestor groups/projects. Under a status filter, ancestor
  // name matches alone do not expose empty groups; name and status filters may still combine.
  const { visGroups, visProjects } = useMemo(() => {
    if (!filtering) {
      return {
        visGroups: null as Set<string> | null,
        visProjects: null as Set<string> | null,
      };
    }
    const sessionMatch = (s: Session) =>
      (!filter || s.name.toLowerCase().includes(filter)) && attributeMatch(s);
    // A group/project qualifies on its own when its name matches the search text or it carries the filtered marker.
    const nodeSelfMatch = (name: string, mark?: string | null) =>
      (!statusFilter && filter.length > 0 && name.toLowerCase().includes(filter)) ||
      markHit(mark);
    const subMemo = new Map<string, boolean>();
    const subtreeVisible = (s: Session): boolean => {
      const c = subMemo.get(s.id);
      if (c !== undefined) return c;
      const vis =
        sessionMatch(s) ||
        sessions.some((k) => k.parentSessionId === s.id && subtreeVisible(k));
      subMemo.set(s.id, vis);
      return vis;
    };
    const memo = new Map<string, boolean>();
    const groupVisible = (g: Group): boolean => {
      const cached = memo.get(g.id);
      if (cached !== undefined) return cached;
      const vis =
        nodeSelfMatch(g.name, g.mark) ||
        sessions.some(
          (s) => !s.parentSessionId && s.groupId === g.id && subtreeVisible(s),
        ) ||
        groups.some((c) => c.parentGroupId === g.id && groupVisible(c));
      memo.set(g.id, vis);
      return vis;
    };
    const visG = new Set<string>();
    for (const g of groups) if (groupVisible(g)) visG.add(g.id);
    const visP = new Set<string>();
    for (const p of projects) {
      const vis =
        nodeSelfMatch(p.name, p.mark) ||
        sessions.some(
          (s) =>
            !s.parentSessionId &&
            s.projectId === p.id &&
            !s.groupId &&
            subtreeVisible(s),
        ) ||
        groups.some((g) => g.projectId === p.id && visG.has(g.id));
      if (vis) visP.add(p.id);
    }
    return { visGroups: visG, visProjects: visP };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filtering, filter, statusFiltering, statusFilterIds, markFilter, groups, sessions, projects]);

  // A name match propagates visibility downward so a matching node's complete subtree remains expandable even when
  // descendants lack the term. Marker propagation is added only for projects/groups below: a marked session keeps
  // its ancestor chain visible, but must not pull unrelated descendants into the marker result.
  const nameHit = (name: string) =>
    !statusFiltering && filter.length > 0 && name.toLowerCase().includes(filter);
  const containerHit = (name: string, mark?: string | null) =>
    nameHit(name) || markHit(mark);

  const sessionMatch = (s: Session) =>
    (!filter || s.name.toLowerCase().includes(filter)) && attributeMatch(s);
  const sessionSubtreeVisible = (s: Session): boolean =>
    sessionMatch(s) ||
    sessions.some((c) => c.parentSessionId === s.id && sessionSubtreeVisible(c));
  const sessionVisible = (s: Session) => !filtering || sessionSubtreeVisible(s);

  // Flattened visible order for Shift-range selection.
  const flat = useMemo(() => {
    const out: SelNode[] = [];
    const sessionWalk = (s: Session, ancestorMatched: boolean) => {
      out.push({ id: s.id, kind: "session" });
      const expanded = filtering ? true : !nodeCollapsed(s.id, s.collapsed);
      if (!expanded) return;
      const matched = ancestorMatched || nameHit(s.name);
      let kids = sessions.filter((c) => c.parentSessionId === s.id);
      if (filtering && !matched) kids = kids.filter((c) => sessionVisible(c));
      for (const k of kids) sessionWalk(k, matched);
    };
    const walk = (
      projectId: string,
      parentGroupId: string | null,
      ancestorMatched: boolean,
    ) => {
      let childGroups = groups.filter(
        (g) =>
          g.projectId === projectId &&
          (parentGroupId ? g.parentGroupId === parentGroupId : !g.parentGroupId),
      );
      if (filtering && !ancestorMatched && visGroups) {
        childGroups = childGroups.filter((g) => visGroups.has(g.id));
      }
      for (const g of childGroups) {
        out.push({ id: g.id, kind: "group" });
        const expanded = filtering ? true : !nodeCollapsed(g.id, g.collapsed);
        if (expanded) walk(projectId, g.id, ancestorMatched || containerHit(g.name, g.mark));
      }
      let childSessions = sessions.filter(
        (s) =>
          !s.parentSessionId &&
          s.projectId === projectId &&
          (parentGroupId ? s.groupId === parentGroupId : !s.groupId),
      );
      if (filtering && !ancestorMatched) {
        childSessions = childSessions.filter((s) => sessionVisible(s));
      }
      for (const s of childSessions) sessionWalk(s, ancestorMatched);
    };
    const visP =
      filtering && visProjects
        ? projects.filter((p) => visProjects.has(p.id))
        : projects;
    for (const p of visP) {
      out.push({ id: p.id, kind: "project" });
      const expanded = filtering ? true : !nodeCollapsed(p.id, p.collapsed);
      if (expanded) walk(p.id, null, containerHit(p.name, p.mark));
    }
    return out;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    projects, groups, sessions, filter, statusFiltering, statusFilterIds, markFilter,
    visGroups, visProjects, collapsedOverrides,
  ]);

  // Flatten the visible tree, including pinned temporary draft types, into virtualized render rows matching the old
  // recursive order/depth. Keep this separate from persistent-only `flat`; temporary rows cannot enter range selection.
  const rows = useMemo(() => {
    const out: TreeRow[] = [];
    // Session subtree, matching the former renderSessionNodes order.
    const sessionWalk = (s: Session, depth: number, ancestorMatched: boolean) => {
      const matched = ancestorMatched || nameHit(s.name);
      let kids = sessions.filter((c) => c.parentSessionId === s.id);
      if (filtering && !matched) kids = kids.filter((c) => sessionVisible(c));
      const hasKids = kids.length > 0;
      const expanded = filtering ? true : !nodeCollapsed(s.id, s.collapsed);
      out.push({ kind: "session", id: s.id, session: s, depth, hasKids, expanded });
      if (hasKids && expanded) {
        for (const k of kids) sessionWalk(k, depth + 1, matched);
      }
    };
    // Groups followed by top-level sessions at the same depth, matching renderChildren.
    const walkChildren = (
      projectId: string,
      parentGroupId: string | null,
      depth: number,
      ancestorMatched: boolean,
    ) => {
      let childGroups = groups.filter(
        (g) =>
          g.projectId === projectId &&
          (parentGroupId ? g.parentGroupId === parentGroupId : !g.parentGroupId),
      );
      if (filtering && !ancestorMatched && visGroups) {
        childGroups = childGroups.filter((g) => visGroups.has(g.id));
      }
      for (const g of childGroups) {
        const expanded = filtering ? true : !nodeCollapsed(g.id, g.collapsed);
        out.push({ kind: "group", id: g.id, group: g, depth, expanded });
        if (expanded) {
          walkChildren(projectId, g.id, depth + 1, ancestorMatched || containerHit(g.name, g.mark));
        }
      }
      let childSessions = sessions.filter(
        (s) =>
          !s.parentSessionId &&
          s.projectId === projectId &&
          (parentGroupId ? s.groupId === parentGroupId : !s.groupId),
      );
      if (filtering && !ancestorMatched) {
        childSessions = childSessions.filter((s) => sessionVisible(s));
      }
      for (const s of childSessions) sessionWalk(s, depth, ancestorMatched);
    };

    // Project followed by its group/session subtree, matching visibleProjects.map.
    const visP =
      filtering && visProjects ? projects.filter((p) => visProjects.has(p.id)) : projects;
    for (const p of visP) {
      const expanded = filtering ? true : !nodeCollapsed(p.id, p.collapsed);
      out.push({ kind: "project", id: p.id, project: p, expanded });
      if (expanded) walkChildren(p.id, null, 1, containerHit(p.name, p.mark));
    }
    return out;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    projects, groups, sessions, filter, statusFiltering, statusFilterIds, markFilter,
    visGroups, visProjects, filtering, collapsedOverrides,
  ]);

  // Exact row heights are compact 24, regular 28, or comfy 32, reduced by 3 in compact navigation. Supplying this
  // to estimateSize keeps unmeasured offscreen rows and scrollbar proportions accurate; measureElement verifies visible rows.
  const rowH =
    (density === "compact" ? 24 : density === "comfy" ? 32 : 28) -
    (navLayout === "compact" ? 3 : 0);

  // Render only visible rows. Stable ID keys preserve the correct DOM through drag reordering or draft conversion;
  // measureElement covers any difference between estimated and actual heights.
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => rowH,
    overscan: 8,
    getItemKey: (i) => rows[i].id,
  });

  // Clear measurements when density/navigation changes so offscreen rows do not retain stale heights and distort totals.
  useEffect(() => {
    virtualizer.measure();
  }, [rowH, virtualizer]);

  // After `vela <path>` imports or reuses a project, reveal its row and consume the one-shot marker.
  useEffect(() => {
    if (!isPrimary || !revealProjectId) return;
    const index = rows.findIndex((row) => row.kind === "project" && row.id === revealProjectId);
    if (index < 0) return;
    virtualizer.scrollToIndex(index, { align: "auto" });
    setRevealProject(null);
  }, [isPrimary, revealProjectId, rows, setRevealProject, virtualizer]);

  // Autoscroll near list edges so virtualized offscreen drop targets enter the viewport. Listen during capture because
  // row onDragOver stops propagation.
  useEffect(() => {
    const el = scrollEl;
    if (!el) return;
    const EDGE = 40; // Edge zone that triggers autoscroll, in pixels.
    const MAX_STEP = 14; // Maximum pixels scrolled per frame.
    let raf = 0;
    let dir = 0; // -1 up, 0 stopped, 1 down.
    let step = 0;
    const tick = () => {
      if (dir === 0) {
        raf = 0;
        return;
      }
      el.scrollTop += dir * step;
      raf = requestAnimationFrame(tick);
    };
    const ensure = () => {
      if (!raf) raf = requestAnimationFrame(tick);
    };
    const stop = () => {
      dir = 0;
      if (raf) {
        cancelAnimationFrame(raf);
        raf = 0;
      }
    };
    const onDragOver = (e: DragEvent) => {
      const rect = el.getBoundingClientRect();
      const y = e.clientY - rect.top;
      if (y < EDGE) {
        dir = -1;
        step = Math.max(2, Math.ceil(((EDGE - y) / EDGE) * MAX_STEP));
        ensure();
      } else if (y > rect.height - EDGE) {
        dir = 1;
        step = Math.max(2, Math.ceil(((y - (rect.height - EDGE)) / EDGE) * MAX_STEP));
        ensure();
      } else {
        dir = 0;
      }
    };
    const onDragLeave = (e: DragEvent) => {
      // Stop only after leaving the container, not when crossing between rows.
      const rect = el.getBoundingClientRect();
      if (
        e.clientX <= rect.left ||
        e.clientX >= rect.right ||
        e.clientY <= rect.top ||
        e.clientY >= rect.bottom
      ) {
        stop();
      }
    };
    el.addEventListener("dragover", onDragOver, true);
    el.addEventListener("drop", stop, true);
    el.addEventListener("dragleave", onDragLeave, true);
    window.addEventListener("dragend", stop);
    return () => {
      el.removeEventListener("dragover", onDragOver, true);
      el.removeEventListener("drop", stop, true);
      el.removeEventListener("dragleave", onDragLeave, true);
      window.removeEventListener("dragend", stop);
      stop();
    };
  }, [scrollEl]);

  // Reveal the active session after tree clicks, tab switches, or notifications. Suppress reveal for newly created,
  // spawned, or forked sessions to avoid sidebar jumps. Hold a pending target while expanding collapsed ancestors,
  // then rerun after rows update. align:"auto" leaves an already visible row in place.
  const revealPendingRef = useRef<string | null>(null);
  useEffect(() => {
    if (!isPrimary) {
      revealPendingRef.current = null;
      return;
    }
    // Consume the one-shot reveal suppression for new/spawned/forked sessions regardless of match so it cannot linger.
    const st = useTermStore.getState();
    const suppressed = !!st.revealSuppressId && st.revealSuppressId === activeSessionId;
    if (st.revealSuppressId) st.setRevealSuppress(null);
    revealPendingRef.current = suppressed ? null : activeSessionId;
  }, [activeSessionId, isPrimary]);
  useEffect(() => {
    const id = revealPendingRef.current;
    if (!id) return;
    // Do not reveal pinned temporary/draft sessions; creation would pull the viewport to the top, and existing ones are visible.
    if (id in ephemeralSessions) {
      revealPendingRef.current = null;
      return;
    }
    const idx = rows.findIndex((r) => r.kind === "session" && r.id === id);
    if (idx >= 0) {
      virtualizer.scrollToIndex(idx, { align: "auto" });
      revealPendingRef.current = null;
      return;
    }
    // If absent because of filtering or collapsed temporary groups, give up. For persistent sessions, expand only
    // collapsed ancestors (parent session, group, project) and retry after rows change.
    const s = !filtering ? sessionsById.get(id) : undefined;
    if (!s) {
      revealPendingRef.current = null;
      return;
    }
    let changed = false;
    let ps = s.parentSessionId ? sessionsById.get(s.parentSessionId) : undefined;
    while (ps) {
      if (ps.collapsed) {
        void toggleCollapsed("session", ps.id);
        changed = true;
      }
      ps = ps.parentSessionId ? sessionsById.get(ps.parentSessionId) : undefined;
    }
    let g = s.groupId ? groupsById.get(s.groupId) : undefined;
    while (g) {
      if (g.collapsed) {
        void toggleCollapsed("group", g.id);
        changed = true;
      }
      g = g.parentGroupId ? groupsById.get(g.parentGroupId) : undefined;
    }
    const proj = projects.find((p) => p.id === s.projectId);
    if (proj?.collapsed) {
      void toggleCollapsed("project", proj.id);
      changed = true;
    }
    // If all ancestors are expanded and the row is still absent, clear pending state defensively.
    if (!changed) revealPendingRef.current = null;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeSessionId, rows]);

  const rangeSelect = (targetId: string) => {
    const anchor = selectionAnchor ?? targetId;
    const ai = flat.findIndex((n) => n.id === anchor);
    const ti = flat.findIndex((n) => n.id === targetId);
    if (ai < 0 || ti < 0) {
      const node = flat.find((n) => n.id === targetId);
      if (node) setSelection([node], targetId);
      return;
    }
    const [lo, hi] = ai < ti ? [ai, ti] : [ti, ai];
    setSelection(flat.slice(lo, hi + 1), anchor);
  };

  const handleClick = (
    node: SelNode,
    e: React.MouseEvent,
    isSession: boolean,
  ) => {
    if (e.metaKey || e.ctrlKey) {
      toggleSelect(node);
      return;
    }
    if (e.shiftKey) {
      rangeSelect(node.id);
      return;
    }
    selectSingle(node);
    if (isSession) {
      // Selecting a session clears project/group inspection so the right pane follows the active session again.
      setInspectTarget(null);
      // Option/Alt-click opens a new tab; a normal click reuses the current tab.
      openSession(node.id, e.altKey ? { newTab: true } : undefined);
    } else {
      // Selecting a project/group makes the right pane inspect its directory instead of the active session.
      setInspectTarget(node);
      if (!filtering) {
        const shared = node.kind === "project"
          ? projects.find((p) => p.id === node.id)?.collapsed
          : groupsById.get(node.id)?.collapsed;
        toggleNodeCollapsed(node.kind, node.id, shared);
      }
    }
  };

  // Session rows carry only their ID, so look the shared collapse value up here. Draft sessions live outside the
  // database and keep theirs in `ephemeralSessions`.
  const toggleSessionCollapsed = (id: string) => {
    const shared = sessionsById.get(id)?.collapsed ?? ephemeralSessions[id]?.collapsed;
    toggleNodeCollapsed("session", id, shared);
  };

  // Prevent native text selection for Cmd/Ctrl/Shift multi-selection gestures; Shift-click can extend selection
  // despite user-select:none.
  const preventModifierSelect = (e: React.MouseEvent) => {
    if (e.shiftKey || e.metaKey || e.ctrlKey) e.preventDefault();
  };

  const ctx = (node: TreeNodeRef) => (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    // Right-click chooses a menu target without changing tree selection. Unselected nodes receive temporary context
    // highlighting; selected nodes retain batch-menu semantics. This keeps the active session from joining selection implicitly.
    onContext(node, e.clientX, e.clientY);
  };

  // Clone the row under an offscreen viewport-level ancestor so every browser gets the same neutral drag card
  // and batch-count badge, independent of the virtual row's absolute positioning.
  const setRowDragImage = (e: React.DragEvent, batchCount?: number) => {
    const rowEl = e.currentTarget as HTMLElement;
    const rect = rowEl.getBoundingClientRect();
    const clone = rowEl.cloneNode(true) as HTMLElement;
    // Remove selected/active/unread styling so the drag image is a neutral lifted card.
    clone.classList.remove("sel", "active", "unread");
    clone.style.cssText +=
      `;position:fixed;top:-10000px;left:0;margin:0;pointer-events:none;width:${rect.width}px;` +
      `background:var(--bg-2);border:1px solid var(--border-strong);` +
      `border-radius:var(--r-sm);box-shadow:var(--shadow);`;
    // Show an item-count badge for batch dragging.
    if (batchCount && batchCount >= 2) {
      const badge = document.createElement("span");
      badge.textContent = String(batchCount);
      badge.style.cssText =
        "position:absolute;right:6px;top:50%;transform:translateY(-50%);min-width:16px;" +
        "height:16px;padding:0 4px;border-radius:9px;background:var(--accent);color:#fff;" +
        "font-size:10px;line-height:16px;text-align:center;font-weight:600;";
      clone.appendChild(badge);
    }
    document.body.appendChild(clone);
    const ox = Math.max(0, Math.min(e.clientX - rect.left, rect.width));
    const oy = Math.max(0, Math.min(e.clientY - rect.top, rect.height));
    e.dataTransfer.setDragImage(clone, ox, oy);
    // The browser snapshots the drag image by the end of dragstart; remove the offscreen clone next frame.
    setTimeout(() => clone.remove(), 0);
  };
  const onDragStart = (payload: DragPayload) => (e: React.DragEvent) => {
    e.stopPropagation();
    let out = payload;
    // Dragging a selected session carries every selected session in the same project. Mixed-project selection falls
    // back to one item, matching context-menu batch restrictions.
    if (payload.kind === "session" && selectedIds.has(payload.id) && selection.length >= 2) {
      const ids = selection
        .filter((n) => n.kind === "session")
        .map((n) => sessionsById.get(n.id))
        .filter((s): s is Session => !!s && s.projectId === payload.projectId)
        .map((s) => s.id);
      if (ids.length >= 2) out = { ...payload, ids };
    }
    e.dataTransfer.setData("text/plain", JSON.stringify(out));
    e.dataTransfer.effectAllowed = "move";
    setRowDragImage(e, out.ids?.length);
  };
  const readPayload = (e: React.DragEvent): DragPayload | null => {
    try {
      return JSON.parse(e.dataTransfer.getData("text/plain")) as DragPayload;
    } catch {
      return null;
    }
  };
  const isInSubtree = (ancestorId: string, target: Group): boolean => {
    let cur: Group | undefined = target;
    while (cur) {
      if (cur.id === ancestorId) return true;
      cur = cur.parentGroupId ? groupsById.get(cur.parentGroupId) : undefined;
    }
    return false;
  };
  const isSessionInSubtree = (ancestorId: string, target: Session): boolean => {
    let cur: Session | undefined = target;
    while (cur) {
      if (cur.id === ancestorId) return true;
      cur = cur.parentSessionId
        ? sessionsById.get(cur.parentSessionId)
        : undefined;
    }
    return false;
  };
  const calcZone = (e: React.DragEvent, hasCenter: boolean): DragZone => {
    const rect = e.currentTarget.getBoundingClientRect();
    const y = e.clientY - rect.top;
    const h = rect.height;
    if (hasCenter) {
      if (y < h * 0.25) return "top";
      if (y > h * 0.75) return "bottom";
      return "center";
    }
    return y < h * 0.5 ? "top" : "bottom";
  };
  const allowDrop = (e: React.DragEvent, id: string, hasCenter: boolean) => {
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = "move";
    const zone = calcZone(e, hasCenter);
    setDragOver((prev) =>
      prev?.id === id && prev.zone === zone ? prev : { id, zone },
    );
  };
  const sortBetween = (
    siblings: { id: string; sortOrder: number }[],
    targetId: string,
    pos: "before" | "after",
  ): number => {
    const idx = siblings.findIndex((s) => s.id === targetId);
    if (idx < 0) return Date.now();
    if (pos === "before") {
      if (idx === 0) return siblings[0].sortOrder - 1000;
      return Math.floor((siblings[idx - 1].sortOrder + siblings[idx].sortOrder) / 2);
    }
    if (idx === siblings.length - 1) return Date.now();
    return Math.floor((siblings[idx].sortOrder + siblings[idx + 1].sortOrder) / 2);
  };
  const dropOnGroup = (target: Group) => (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const zone = dragOver?.id === target.id ? dragOver.zone : "center";
    setDragOver(null);
    const p = readPayload(e);
    if (!p) return;

    // Batch drop moves all sessions into this group at center or its parent at an edge through one moveMany call.
    if (p.ids && p.ids.length >= 2) {
      const gid = zone === "center" ? target.id : target.parentGroupId ?? null;
      void moveMany(p.ids, target.projectId, gid, null);
      return;
    }

    if (zone === "center") {
      if (p.kind === "session") {
        void moveNode("session", p.id, target.projectId, target.id, null, Date.now());
      } else {
        if (p.projectId !== target.projectId) return;
        if (isInSubtree(p.id, target)) return;
        void moveNode("group", p.id, target.projectId, target.id, null, Date.now());
      }
      return;
    }
    // top/bottom: reorder among sibling groups
    const siblings = groups
      .filter(
        (g) =>
          g.projectId === target.projectId &&
          (g.parentGroupId ?? null) === (target.parentGroupId ?? null),
      )
      .sort((a, b) => a.sortOrder - b.sortOrder);
    const order = sortBetween(siblings, target.id, zone === "top" ? "before" : "after");
    if (p.kind === "group") {
      if (p.projectId !== target.projectId) return;
      if (isInSubtree(p.id, target)) return;
      void moveNode("group", p.id, target.projectId, target.parentGroupId ?? null, null, order);
    } else {
      void moveNode(
        "session", p.id, target.projectId, target.parentGroupId ?? null, null, order,
      );
    }
  };
  const dropOnProject = (projectId: string) => (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const zone = dragOver?.id === projectId ? dragOver.zone : "center";
    setDragOver(null);
    const p = readPayload(e);
    if (!p) return;

    // Batch drop moves all sessions to the ungrouped project root.
    if (p.ids && p.ids.length >= 2) {
      void moveMany(p.ids, projectId, null, null);
      return;
    }

    if (zone === "center") {
      if (p.kind === "session") {
        void moveNode("session", p.id, projectId, null, null, Date.now());
      } else {
        if (p.projectId !== projectId) return;
        void moveNode("group", p.id, projectId, null, null, Date.now());
      }
      return;
    }
    // top/bottom: session/group dropped at edge of project → move into project root
    if (p.kind === "session") {
      void moveNode("session", p.id, projectId, null, null, Date.now());
    } else {
      if (p.projectId !== projectId) return;
      void moveNode("group", p.id, projectId, null, null, Date.now());
    }
  };
  const dropOnSession = (target: Session) => (e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const zone = dragOver?.id === target.id ? dragOver.zone : "center";
    setDragOver(null);
    const p = readPayload(e);
    if (!p) return;

    // Batch drop nests under the target at center or joins its level at an edge. Exclude the target and any session
    // whose descendant is the target to prevent self-moves or cycles.
    if (p.ids && p.ids.length >= 2) {
      const ids = p.ids.filter(
        (id) => id !== target.id && !isSessionInSubtree(id, target),
      );
      if (ids.length === 0) return;
      const parent = zone === "center" ? target.id : target.parentSessionId ?? null;
      void moveMany(ids, target.projectId, target.groupId ?? null, parent);
      return;
    }

    if (p.kind !== "session") return;
    if (p.id === target.id) return;

    if (zone === "center") {
      if (isSessionInSubtree(p.id, target)) return;
      void moveNode(
        "session", p.id, target.projectId, target.groupId ?? null, target.id, Date.now(),
      );
      return;
    }
    // top/bottom: reorder among siblings of target
    const siblings = sessions
      .filter(
        (s) =>
          s.projectId === target.projectId &&
          (s.groupId ?? null) === (target.groupId ?? null) &&
          (s.parentSessionId ?? null) === (target.parentSessionId ?? null),
      )
      .sort((a, b) => a.sortOrder - b.sortOrder);
    const order = sortBetween(siblings, target.id, zone === "top" ? "before" : "after");
    void moveNode(
      "session", p.id, target.projectId, target.groupId ?? null,
      target.parentSessionId ?? null, order,
    );
  };

  const dragStyle = (id: string): React.CSSProperties | undefined => {
    if (!dragOver || dragOver.id !== id) return undefined;
    if (dragOver.zone === "center") {
      return { background: "var(--accent-soft)", outline: "1px solid var(--accent)", outlineOffset: -1 };
    }
    const line = "2px solid var(--accent)";
    if (dragOver.zone === "top") return { borderTop: line };
    return { borderBottom: line };
  };

  // Give SessionRow stable useCallback identities while reading current logic through cbRef. Session switches then
  // do not force memoized rows to rerender merely because callbacks were recreated.
  const cb = {
    handleClick,
    ctx,
    dropOnSession,
    allowDrop,
    onDragStart,
    setDragOver,
    preventModifierSelect,
    toggleSessionCollapsed,
    filtering,
    h,
  };
  const cbRef = useRef(cb);
  cbRef.current = cb;
  const sOnClick = useCallback(
    (node: SelNode, e: React.MouseEvent, isSession: boolean) =>
      cbRef.current.handleClick(node, e, isSession),
    [],
  );
  const sOnContext = useCallback(
    (ref: TreeNodeRef, e: React.MouseEvent) => cbRef.current.ctx(ref)(e),
    [],
  );
  const sOnToggleSession = useCallback((id: string) => {
    if (!cbRef.current.filtering) cbRef.current.toggleSessionCollapsed(id);
  }, []);
  const sOnDragStart = useCallback(
    (payload: DragPayload, e: React.DragEvent) => cbRef.current.onDragStart(payload)(e),
    [],
  );
  const sOnDragOver = useCallback(
    (id: string, e: React.DragEvent) => cbRef.current.allowDrop(e, id, true),
    [],
  );
  const sOnDragLeave = useCallback(
    (id: string) => cbRef.current.setDragOver((d) => (d?.id === id ? null : d)),
    [],
  );
  const sOnDropSession = useCallback(
    (target: Session, e: React.DragEvent) => cbRef.current.dropOnSession(target)(e),
    [],
  );
  const sOnMouseDown = useCallback(
    (e: React.MouseEvent) => cbRef.current.preventModifierSelect(e),
    [],
  );
  const sOnAddSession = useCallback(
    (ref: TreeNodeRef, e: React.MouseEvent) =>
      cbRef.current.h.onAddSession(ref, e.clientX, e.clientY),
    [],
  );
  const sSetRenameVal = useCallback((v: string) => cbRef.current.h.setRenameVal(v), []);
  const sCommitRename = useCallback(() => cbRef.current.h.commitRename(), []);
  const sCancelRename = useCallback(() => cbRef.current.h.cancelRename(), []);

  // Inline rename input shared by all node types.
  const renameInput = (
    <input
      ref={ctrlCharGuard}
      className="rename-input"
      autoFocus
      value={renameVal}
      onClick={(e) => e.stopPropagation()}
      onChange={(e) => setRenameVal(stripControlChars(e.target.value))}
      onKeyDown={(e) => {
        if (e.key === "Enter") commitRename();
        if (e.key === "Escape") cancelRename();
      }}
      onBlur={commitRename}
    />
  );

  // Hover create controls: session/group for projects and groups, child session for sessions.
  const metaButtons = (ref: TreeNodeRef, canAddGroup: boolean) => (
    <span className="meta">
      <span
        className="add"
        title={ref.kind === "session" ? t("tree.newChildSession") : t("tree.newSession")}
        onClick={(e) => {
          e.stopPropagation();
          onAddSession(ref, e.clientX, e.clientY);
        }}
      >
        <Icons.plus size={12} />
      </span>
      {canAddGroup && (
        <span
          className="add"
          title={t("tree.newGroup")}
          onClick={(e) => {
            e.stopPropagation();
            onAddGroup(ref);
          }}
        >
          <Icons.newGroup size={12} />
        </span>
      )}
    </span>
  );

  // Render one flattened row by kind, centralizing JSX formerly spread across three recursive render paths.
  const renderRow = (row: TreeRow): React.ReactNode => {
    switch (row.kind) {
      case "project": {
        const p = row.project;
        const renaming = renamingId === p.id;
        const ref: TreeNodeRef = {
          kind: "project",
          id: p.id,
          name: p.name,
          projectId: p.id,
          groupId: null,
        };
        return (
          <div
            className={
              "row project" +
              (selectedIds.has(p.id) ? " sel" : "") +
              (contextId === p.id ? " context" : "")
            }
            style={{ paddingLeft: 6, ...dragStyle(p.id) }}
            onDragOver={(e) => allowDrop(e, p.id, true)}
            onDragLeave={() => setDragOver((d) => (d?.id === p.id ? null : d))}
            onDrop={dropOnProject(p.id)}
            onMouseDown={preventModifierSelect}
            onClick={(e) => handleClick({ id: p.id, kind: "project" }, e, false)}
            onContextMenu={ctx(ref)}
          >
            <span
              className="tw"
              onClick={(e) => {
                e.stopPropagation();
                if (!filtering) toggleNodeCollapsed("project", p.id, p.collapsed);
              }}
            >
              <Chevron open={row.expanded} />
            </span>
            {/* A collection has no folder behind it, so it carries its own icon and hover label. */}
            <span
              className="ic"
              style={{ color: "var(--text-dim)" }}
              title={isVirtualProject(p) ? t("collection.tag") : p.rootPath}
            >
              {isVirtualProject(p) ? <Icons.layers size={15} /> : <Icons.project size={15} />}
            </span>
            {renaming ? (
              renameInput
            ) : (
              <>
                <NodeMarkBadge mark={p.mark} />
                <span className="nm">{p.name}</span>
              </>
            )}
            {metaButtons(ref, true)}
          </div>
        );
      }
      case "group": {
        const g = row.group;
        const renaming = renamingId === g.id;
        const ref: TreeNodeRef = {
          kind: "group",
          id: g.id,
          name: g.name,
          projectId: g.projectId,
          groupId: g.parentGroupId ?? null,
        };
        return (
          <div
            className={
              "row group" +
              (selectedIds.has(g.id) ? " sel" : "") +
              (contextId === g.id ? " context" : "")
            }
            style={{ paddingLeft: 6 + row.depth * 13, ...dragStyle(g.id) }}
            draggable={!filtering && !renaming}
            onDragStart={onDragStart({ kind: "group", id: g.id, projectId: g.projectId })}
            onDragOver={(e) => allowDrop(e, g.id, true)}
            onDragLeave={() => setDragOver((d) => (d?.id === g.id ? null : d))}
            onDrop={dropOnGroup(g)}
            onMouseDown={preventModifierSelect}
            onClick={(e) => handleClick({ id: g.id, kind: "group" }, e, false)}
            onContextMenu={ctx(ref)}
          >
            <span
              className="tw"
              onClick={(e) => {
                e.stopPropagation();
                if (!filtering) toggleNodeCollapsed("group", g.id, g.collapsed);
              }}
            >
              <Chevron open={row.expanded} />
            </span>
            <span className="ic" style={{ color: "var(--text-dim)" }}>
              {row.expanded ? <Icons.folderOpen size={15} /> : <Icons.folder size={15} />}
              {/* Worktree badge: at the bottom right of the folder icon, carried by .ic which is already
                  position:relative. It matches the session icon badge, follows the light/dark theme,
                  takes no inline width and does not cover the group name. */}
              {g.worktreePath && <GroupWorktreeBadge path={g.worktreePath} />}
            </span>
            {renaming ? (
              renameInput
            ) : (
              <>
                <NodeMarkBadge mark={g.mark} />
                <span className="nm">{g.name}</span>
              </>
            )}
            {metaButtons(ref, true)}
          </div>
        );
      }
      case "session": {
        // Session rows, including temporary terminals and split children, use memoized SessionRow with isolated
        // high-frequency subscriptions and stable callbacks.
        const s = row.session;
        const renaming = renamingId === s.id;
        return (
          <SessionRow
            session={s}
            rootPath={projects.find((pr) => pr.id === s.projectId)?.rootPath ?? null}
            depth={row.depth}
            hasKids={row.hasKids}
            expanded={row.expanded}
            context={contextId === s.id}
            renaming={renaming}
            renameVal={renaming ? renameVal : undefined}
            draggable={!filtering && !renaming}
            dragHighlight={dragStyle(s.id)}
            onRowClick={sOnClick}
            onRowContext={sOnContext}
            onToggle={sOnToggleSession}
            onDragStartRow={sOnDragStart}
            onDragOverRow={sOnDragOver}
            onDragLeaveRow={sOnDragLeave}
            onDropRow={sOnDropSession}
            onMouseDownRow={sOnMouseDown}
            onAddSession={sOnAddSession}
            setRenameVal={sSetRenameVal}
            commitRename={sCommitRename}
            cancelRename={sCancelRename}
          />
        );
      }
    }
  };

  // Before the first tree load resolves the store is still empty, so rendering the empty state here would flash
  // "no projects yet" on every startup and read as a lost workspace. Keep the panel blank until data arrives.
  if (!treeLoaded) {
    return (
      <div className="tree">
        {showLoadingHint && (
          <div
            style={{
              padding: "12px",
              display: "flex",
              alignItems: "center",
              gap: 6,
              fontSize: 12,
              color: "var(--text-faint)",
            }}
          >
            <span className="spin" style={{ display: "inline-flex" }}>
              <Icons.restart size={12} />
            </span>
            {t("common.loading")}
          </div>
        )}
      </div>
    );
  }

  // Show all three project entry points when no projects exist; draft terminals appear only in center tabs.
  if (projects.length === 0) {
    return (
      <div
        style={{
          padding: "24px 16px",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          gap: 12,
          textAlign: "center",
        }}
      >
        <div style={{ fontSize: 12, color: "var(--text-faint)", lineHeight: 1.8 }}>
          {t("tree.noProjectsPre")}
          <kbd className="hint-kbd">{formatCombo(openProjectCombo)}</kbd>
          {t("tree.noProjectsPost")}
        </div>
        <button className="empty-action" onClick={() => setCreateProjectModalOpen(true)}>
          <Icons.folderPlus size={14} />
          {t("tree.createProject")}
        </button>
        <button className="empty-action" onClick={() => void importProject()}>
          <Icons.folderOpen size={14} />
          {t("tree.openProject")}
        </button>
        <button className="empty-action" onClick={() => setCloneModalOpen(true)}>
          <Icons.git size={14} />
          {t("tree.cloneProject")}
        </button>
      </div>
    );
  }

  // Under filtering, empty rows means no visible project because temporary groups are excluded; show an empty hint.
  const showEmptyHint = filtering && rows.length === 0;

  return (
    <div ref={setParent} className="tree">
      {showEmptyHint ? (
        <div style={{ padding: "12px", fontSize: 12, color: "var(--text-faint)" }}>
          {statusFiltering && !filter ? t("tree.noAttention") : t("tree.noMatch")}
        </div>
      ) : (
        // Establish total scroll height for the tree and custom scrollbar. Position rows through layout `top`
        // instead of a compositor transform: WKWebView can retain stale hit-test coordinates for transformed
        // virtual rows after scrolling or tree updates, making hover/click land on a visually different row.
        <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
          {virtualizer.getVirtualItems().map((vi) => (
            <div
              key={vi.key}
              data-index={vi.index}
              ref={virtualizer.measureElement}
              style={{
                position: "absolute",
                top: vi.start,
                left: 0,
                right: 0,
              }}
            >
              {renderRow(rows[vi.index])}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
