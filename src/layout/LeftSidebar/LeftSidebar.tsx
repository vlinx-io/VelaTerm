//! Vlinx-style sidebar containing the Workspace header, search, project tree, context menus, and dialogs.
//! Session actions and dialogs are shared with the center tab bar through ../sessionMenu. This component
//! retains tree-specific project/group menus, multiselection actions, inline rename, hover creation, and filtering.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ContextMenu, type MenuItem } from "../../components/ContextMenu";
import Icons from "../../components/Icons";
import { useT } from "../../i18n";
import {
  type SidebarTreeView,
  useTermStore,
} from "../../store/termStore";
import { isWorktreeGone } from "../../hooks/useGitBranch";
import {
  countByAgentState,
  type AgentState,
  type NodeKind,
} from "../../types";
import { MARK_LABEL_KEYS, NODE_MARKS, normalizeMark } from "../../marks";
import { useSessionMenu } from "../sessionMenu";
import { GlobalSearch } from "../GlobalSearch/GlobalSearch";
import { ArchivePanel } from "./ArchivePanel";
import {
  ProjectTree,
  type TreeHandlers,
  type TreeNodeRef,
} from "./ProjectTree";
import {
  collectSidebarViewIds,
  computeSidebarDividers,
  computeSidebarViewLayout,
  type SidebarDividerInfo,
  type SidebarTreeTab,
  type SidebarViewRect,
} from "./sidebarTreeLayout";

/** Status filters: working (pulsing green), attention (pulsing yellow), and replied (magenta). */
const STATUS_FILTERS: {
  st: AgentState;
  color: string;
  pulse: boolean;
  labelKey: "tree.filterWorking" | "tree.filterAsking" | "tree.filterWaiting";
}[] = [
  { st: "working", color: "var(--status-working)", pulse: true, labelKey: "tree.filterWorking" },
  { st: "asking", color: "var(--status-asking)", pulse: true, labelKey: "tree.filterAsking" },
  { st: "waiting", color: "var(--status-waiting)", pulse: false, labelKey: "tree.filterWaiting" },
];

/** Isolated filter button/dropdown subscribing to high-frequency runtime/notification changes. Agent updates
 * rerender only this control instead of LeftSidebar and its large ProjectTree, removing a major source of lag
 * during tab closure, switching, and typing. The dropdown holds a multi-select session-status section and an
 * independent single-select node-marker section. */
function TreeFilter({ view }: { view: SidebarTreeView }) {
  const t = useT();
  const sessions = useTermStore((s) => s.sessions);
  const groups = useTermStore((s) => s.groups);
  const projects = useTermStore((s) => s.projects);
  const runtimes = useTermStore((s) => s.runtimes);
  const notifications = useTermStore((s) => s.notifications);
  const dynamicStatusFilter = useTermStore((s) => s.dynamicStatusFilter);
  const setStatusFilter = useTermStore((s) => s.setSidebarTreeViewStatusFilter);
  const appendStatusMatches = useTermStore((s) => s.appendSidebarTreeViewStatusMatches);
  const setMarkFilter = useTermStore((s) => s.setSidebarTreeViewMarkFilter);
  const [filterOpen, setFilterOpen] = useState(false);
  const { statusFilter, markFilter } = view;

  // Per-state session counts shared with the bottom status bar for consistent totals.
  const statusCounts = useMemo(
    () => countByAgentState(sessions, runtimes, notifications),
    [sessions, runtimes, notifications],
  );

  // Marker counts span all three node kinds, matching what the filter actually keeps visible. Markers nobody uses
  // still appear so the palette stays a stable target.
  const markCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const node of [...projects, ...groups, ...sessions]) {
      const m = normalizeMark(node.mark);
      if (m) counts[m] = (counts[m] ?? 0) + 1;
    }
    return counts;
  }, [projects, groups, sessions]);

  const active = statusFilter !== null || markFilter !== null;
  const selectedStatus = statusFilter?.length === 1
    ? STATUS_FILTERS.find((f) => f.st === statusFilter[0])
    : null;

  // When enabled, runtime and unread changes add newly matching sessions to the retained snapshot. Existing
  // members keep the original stable-filter behavior and are never removed merely because their status changes.
  useEffect(() => {
    if (dynamicStatusFilter && statusFilter) appendStatusMatches(view.id);
  }, [
    appendStatusMatches,
    dynamicStatusFilter,
    notifications,
    runtimes,
    sessions,
    statusFilter,
    view.id,
  ]);

  return (
    <div className="filter-wrap">
      <button
        className={active ? "icon-btn sm on" : "icon-btn sm"}
        title={t("tree.filterStatus")}
        onClick={() => setFilterOpen((o) => !o)}
      >
        <Icons.sliders size={14} />
        {/* One status keeps its color badge; multiple statuses use a count. The marker filter only lights the button
            up: a second badge would not fit next to the status one, and the dropdown already names the marker. */}
        {selectedStatus ? (
          <span
            className={
              "filter-badge dot" +
              (selectedStatus.pulse ? " vlx-status-pulse" : "")
            }
            style={{
              background: selectedStatus.color,
            }}
          />
        ) : statusFilter ? (
          <span className="filter-badge">{statusFilter.length}</span>
        ) : null}
      </button>
      {filterOpen && (
        <>
          <div className="dropdown-mask" onClick={() => setFilterOpen(false)} />
          <div className="filter-menu">
            <div className="filter-menu-title">{t("tree.filterStatusSection")}</div>
            {STATUS_FILTERS.map(({ st, color, pulse, labelKey }) => {
              const on = statusFilter?.includes(st) ?? false;
              return (
                <button
                  key={st}
                  className={on ? "filter-menu-item on" : "filter-menu-item"}
                  onClick={() => setStatusFilter(view.id, st)}
                >
                  <span className="ck">{on && <Icons.check size={12} />}</span>
                  <span
                    className={pulse ? "dot vlx-status-pulse" : "dot"}
                    style={{ background: color }}
                  />
                  <span className="lbl">{t(labelKey)}</span>
                  <span className="cnt">{statusCounts[st]}</span>
                </button>
              );
            })}
            <div className="filter-menu-title">{t("tree.filterMarkSection")}</div>
            {NODE_MARKS.map((m) => {
              const on = markFilter === m;
              return (
                <button
                  key={m}
                  className={on ? "filter-menu-item on" : "filter-menu-item"}
                  onClick={() => {
                    setMarkFilter(view.id, m);
                    setFilterOpen(false);
                  }}
                >
                  <span className="ck">{on && <Icons.check size={12} />}</span>
                  <span className="mark-emoji">{m}</span>
                  <span className="lbl">{t(MARK_LABEL_KEYS[m])}</span>
                  <span className="cnt">{markCounts[m] ?? 0}</span>
                </button>
              );
            })}
          </div>
        </>
      )}
    </div>
  );
}

/** One independently filtered projection of the shared tree. Every pane can be split again. */
function SidebarTreePane({
  view,
  isPrimary,
  split,
  active,
  onActivate,
  onSplitDown,
  onClose,
  treeHandlers,
}: {
  view: SidebarTreeView;
  isPrimary: boolean;
  split: boolean;
  active: boolean;
  onActivate: () => void;
  onSplitDown: () => void;
  onClose: () => void;
  treeHandlers: Omit<TreeHandlers, "view" | "isPrimary">;
}) {
  const t = useT();
  const setTreeFilter = useTermStore((s) => s.setSidebarTreeViewFilter);
  const refreshStatusMatches = useTermStore((s) => s.refreshSidebarTreeViewStatusMatches);
  const treeWrapRef = useRef<HTMLDivElement>(null);

  return (
    <section
      className={
        "sidebar-tree-pane" +
        (split ? " split" : "") +
        (active ? " active" : "")
      }
      onMouseDown={onActivate}
    >
      <div className="searchbar sidebar-tree-search">
        <div className="box">
          <Icons.search size={13} />
          <input
            placeholder={t("tree.searchPlaceholder")}
            value={view.treeFilter}
            onChange={(event) => setTreeFilter(view.id, event.target.value)}
          />
          {view.treeFilter && (
            <button
              className="search-clear"
              title={t("tree.clearSearch")}
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => setTreeFilter(view.id, "")}
            >
              <Icons.x size={12} />
            </button>
          )}
        </div>
        <TreeFilter view={view} />
        {split && !isPrimary && (
          <button
            className="icon-btn sm sidebar-tree-close"
            title={t("common.close")}
            aria-label={t("common.close")}
            onClick={(event) => {
              event.stopPropagation();
              onClose();
            }}
          >
            <Icons.x size={13} />
          </button>
        )}
      </div>
      <div ref={treeWrapRef} className="sidebar-tree-body">
        <ProjectTree view={view} isPrimary={isPrimary} {...treeHandlers} />
        <TreeScrollbar wrapRef={treeWrapRef} />
      </div>
      <div className="sidebar-tree-footer">
        <button
          className="icon-btn sm sidebar-tree-refresh"
          title={t("tree.refreshStatusFilter")}
          aria-label={t("tree.refreshStatusFilter")}
          disabled={!view.statusFilter}
          onClick={(event) => {
            event.stopPropagation();
            refreshStatusMatches(view.id);
          }}
        >
          <Icons.restart size={14} />
        </button>
        <button
          className="icon-btn sm sidebar-tree-split"
          title={t("tree.viewSplitDown")}
          aria-label={t("tree.viewSplitDown")}
          onClick={(event) => {
            event.stopPropagation();
            onSplitDown();
          }}
        >
          <Icons.splitH size={14} />
        </button>
      </div>
    </section>
  );
}

const sidebarRectStyle = (rect: SidebarViewRect): React.CSSProperties => ({
  position: "absolute",
  left: `${rect.left}%`,
  top: `${rect.top}%`,
  width: `${rect.width}%`,
  height: `${rect.height}%`,
});

/** Draggable divider for one node in the recursive sidebar split tree. */
function SidebarTreeDivider({
  tabId,
  info,
  stageRef,
}: {
  tabId: string;
  info: SidebarDividerInfo;
  stageRef: React.RefObject<HTMLDivElement | null>;
}) {
  const t = useT();
  const resizeSplit = useTermStore((s) => s.resizeSidebarTreeSplit);
  const horizontal = info.dir === "horizontal";

  const startDrag = (event: React.MouseEvent) => {
    event.preventDefault();
    const stage = stageRef.current;
    if (!stage) return;
    const stageRect = stage.getBoundingClientRect();
    const parentPixels = horizontal
      ? (stageRect.width * info.parentRect.width) / 100
      : (stageRect.height * info.parentRect.height) / 100;
    if (parentPixels <= 0) return;
    const startPosition = horizontal ? event.clientX : event.clientY;
    const [first, second] = info.sizes;
    const divider = event.currentTarget as HTMLElement;
    const previousCursor = document.body.style.cursor;
    const previousUserSelect = document.body.style.userSelect;
    divider.classList.add("dragging");

    const move = (moveEvent: MouseEvent) => {
      const position = horizontal ? moveEvent.clientX : moveEvent.clientY;
      let delta = ((position - startPosition) / parentPixels) * 100;
      delta = Math.max(-(first - 10), Math.min(second - 10, delta));
      resizeSplit(tabId, info.paneId, [first + delta, second - delta]);
    };
    const stop = () => {
      divider.classList.remove("dragging");
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", stop);
      window.removeEventListener("blur", stop);
      document.body.style.cursor = previousCursor;
      document.body.style.userSelect = previousUserSelect;
    };
    document.body.style.cursor = horizontal ? "col-resize" : "row-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", stop);
    window.addEventListener("blur", stop);
  };

  const style: React.CSSProperties = horizontal
    ? {
        position: "absolute",
        left: `${info.leftPct}%`,
        top: `${info.topPct}%`,
        height: `${info.lengthPct}%`,
        transform: "translateX(-50%)",
      }
    : {
        position: "absolute",
        left: `${info.leftPct}%`,
        top: `${info.topPct}%`,
        width: `${info.lengthPct}%`,
        transform: "translateY(-50%)",
      };

  return (
    <div
      className={"sidebar-tree-divider " + (horizontal ? "horizontal" : "vertical")}
      role="separator"
      aria-orientation={horizontal ? "vertical" : "horizontal"}
      title={t("splitter.dragToResize")}
      style={style}
      onMouseDown={startDrag}
    />
  );
}

export function LeftSidebar() {
  const t = useT();
  const width = useTermStore((s) => s.leftWidth);
  const importProject = useTermStore((s) => s.importProject);
  const setCloneModalOpen = useTermStore((s) => s.setCloneModalOpen);
  const renameNode = useTermStore((s) => s.renameNode);
  const openSession = useTermStore((s) => s.openSession);
  const sidebarTreeViews = useTermStore((s) => s.sidebarTreeViews);
  const sidebarTreeTabs = useTermStore((s) => s.sidebarTreeTabs);
  const primaryViewId = useTermStore((s) => s.primarySidebarTreeViewId);
  const activeViewId = useTermStore((s) => s.activeSidebarTreeViewId);
  const splitTreeView = useTermStore((s) => s.splitSidebarTreeView);
  const deleteTreeView = useTermStore((s) => s.deleteSidebarTreeView);
  const setActiveTreeView = useTermStore((s) => s.setActiveSidebarTreeView);
  const legacyTreeFilter = useTermStore((s) => s.treeFilter);
  const legacyStatusFilter = useTermStore((s) => s.statusFilter);
  const legacyStatusFilterIds = useTermStore((s) => s.statusFilterIds);
  const legacyMarkFilter = useTermStore((s) => s.markFilter);
  const selection = useTermStore((s) => s.selection);
  const clearSelection = useTermStore((s) => s.clearSelection);
  const archiveMany = useTermStore((s) => s.archiveMany);
  const archiveGroup = useTermStore((s) => s.archiveGroup);
  const clearNodeWorktree = useTermStore((s) => s.clearNodeWorktree);
  const archiveOpen = useTermStore((s) => s.archiveOpen);
  const setArchiveOpen = useTermStore((s) => s.setArchiveOpen);
  const globalSearchOpen = useTermStore((s) => s.globalSearchOpen);
  const setGlobalSearchOpen = useTermStore((s) => s.setGlobalSearchOpen);
  const hasNotifications = useTermStore((s) => Object.keys(s.notifications).length > 0);
  const clearAllNotifications = useTermStore((s) => s.clearAllNotifications);

  // Keep shallow component mocks and pre-migration in-memory states renderable while the store initializes.
  const views: SidebarTreeView[] = sidebarTreeViews?.length
    ? sidebarTreeViews
    : [{
        id: "main",
        name: t("tree.viewMainName"),
        treeFilter: legacyTreeFilter ?? "",
        statusFilter: legacyStatusFilter ?? null,
        statusFilterIds: legacyStatusFilterIds ?? null,
        markFilter: legacyMarkFilter ?? null,
        collapsedOverrides: null,
      }];
  const effectivePrimaryViewId =
    views.some((view) => view.id === primaryViewId) ? primaryViewId : views[0].id;
  const effectiveActiveViewId =
    views.some((view) => view.id === activeViewId) ? activeViewId : effectivePrimaryViewId;
  const fallbackTabs: SidebarTreeTab[] = views.map((view) => ({
    id: `fallback-${view.id}`,
    root: { kind: "leaf", paneId: `fallback-pane-${view.id}`, viewId: view.id },
    activeViewId: view.id,
  }));
  const tabs = sidebarTreeTabs?.length ? sidebarTreeTabs : fallbackTabs;
  const activeTab = tabs.find((tab) =>
    collectSidebarViewIds(tab.root).includes(effectiveActiveViewId)) ??
    tabs.find((tab) => collectSidebarViewIds(tab.root).includes(effectivePrimaryViewId)) ??
    tabs[0];
  const activeLayout = useMemo(
    () => computeSidebarViewLayout(activeTab.root),
    [activeTab.root],
  );
  const activeDividers = useMemo(
    () => computeSidebarDividers(activeTab.root),
    [activeTab.root],
  );
  const activeTabHasSplits = activeTab.root.kind === "split";
  const treeStageRef = useRef<HTMLDivElement>(null);

  // Three draft sets route context menus and inline rename to temporary actions and in-memory renameScratch.
  const ephemeralSessions = useTermStore((s) => s.ephemeralSessions);
  const browserTabs = useTermStore((s) => s.browserTabs);
  const docTabs = useTermStore((s) => s.docTabs);
  const renameScratch = useTermStore((s) => s.renameScratch);

  // Session action menu and dialogs shared with the tab bar through a common hook.
  const {
    newSessionItems,
    buildSessionItems,
    buildScratchItems,
    buildMoveToMany,
    buildGitItems,
    buildMarkItem,
    openDialog,
    dialogs,
  } = useSessionMenu();

  const [menu, setMenu] = useState<{
    node: TreeNodeRef;
    viewId: string;
    x: number;
    y: number;
  } | null>(null);
  const [newSessionMenu, setNewSessionMenu] = useState<{
    items: MenuItem[];
    x: number;
    y: number;
  } | null>(null);
  // Inline rename state.
  const [renaming, setRenaming] = useState<{
    id: string;
    kind: NodeKind;
    viewId: string;
  } | null>(null);
  const [renameVal, setRenameVal] = useState("");
  const startRename = (node: TreeNodeRef) => {
    const viewId = menu?.viewId ?? effectiveActiveViewId;
    setRenaming({ id: node.id, kind: node.kind, viewId });
    setRenameVal(node.name);
  };
  const commitRename = () => {
    // Strip control characters again at the final boundary. WKWebView may insert U+001C on edge-case
    // arrow input (see ProjectTree); never persist a name containing replacement boxes.
    const name = renameVal.replace(/[\u0000-\u001F\u007F-\u009F]/g, "").trim();
    if (renaming && name) {
      const id = renaming.id;
      // Rename temporary session/browser/document drafts in memory; persist formal nodes through renameNode.
      if (ephemeralSessions[id] || browserTabs[id] || docTabs[id]) {
        renameScratch(id, name);
      } else {
        void renameNode(renaming.kind, id, name);
      }
    }
    setRenaming(null);
  };
  const cancelRename = () => setRenaming(null);

  // Hover add for sessions opens a type menu scoped to the node kind.
  const onAddSession = (node: TreeNodeRef, x: number, y: number) => {
    const projectId = node.projectId;
    let groupId: string | null = null;
    let parent: string | null = null;
    if (node.kind === "group") groupId = node.id;
    else if (node.kind === "session") {
      groupId = node.groupId;
      parent = node.id;
    }
    // Put New Terminal Session first in every project/group/session hover-add menu, and in group context
    // menus, for convenient center-pane draft terminals.
    setNewSessionMenu({ x, y, items: newSessionItems(projectId, groupId, parent, { withTerminal: true }) });
  };

  // Hover add creates top-level groups under projects or child groups under groups.
  const onAddGroup = (node: TreeNodeRef) => {
    const parentGroupId = node.kind === "group" ? node.id : null;
    openDialog({ type: "newGroup", projectId: node.projectId, parentGroupId });
  };

  const buildMenu = (node: TreeNodeRef): MenuItem[] => {
    // Multiselection batch menu.
    if (selection.length >= 2 && selection.some((s) => s.id === node.id)) {
      const items: MenuItem[] = [];
      if (selection.some((s) => s.kind === "session")) {
        items.push({
          label: t("tree.openSelected"),
          onClick: () => {
            // Open each selection in a separate new tab so single-tab reuse does not leave only the last.
            selection
              .filter((s) => s.kind === "session")
              .forEach((s) => openSession(s.id, { newTab: true }));
            clearSelection();
          },
        });
        // Refresh Status re-checks each selected session against this pane's status filter, mirroring the
        // single-session item. It appears only when the pane the menu was opened from filters by status.
        const statusView = sidebarTreeViews.find(
          (v) => v.id === (menu?.viewId ?? effectiveActiveViewId),
        );
        if (statusView?.statusFilter) {
          items.push({
            label: t("tree.refreshStatusMatch"),
            icon: <Icons.restart size={14} />,
            onClick: () => {
              const st = useTermStore.getState();
              selection
                .filter((s) => s.kind === "session")
                .forEach((s) => st.refreshSidebarTreeViewStatusMatch(statusView.id, s.id));
              clearSelection();
            },
          });
        }
        // Offer Move Selected To only when every selected session belongs to one project.
        const moveItem = buildMoveToMany(
          selection.filter((s) => s.kind === "session").map((s) => s.id),
        );
        if (moveItem) items.push(moveItem);
        items.push({
          label: t("tree.archiveSelected"),
          onClick: () => {
            // archiveMany processes sequentially and reloads once. Concurrent per-session archiveSession
            // calls caused loadTree races and React #185. archiveMany also clears selection.
            void archiveMany(
              selection.filter((s) => s.kind === "session").map((s) => s.id),
            );
          },
        });
        items.push({ label: "", separator: true });
      }
      items.push({
        label: t("tree.deleteSelected", selection.length),
        danger: true,
        onClick: () => openDialog({ type: "confirmDeleteMany", nodes: selection }),
      });
      return items;
    }

    const sep: MenuItem = { label: "", separator: true };
    const rename: MenuItem = { label: t("common.rename"), onClick: () => startRename(node) };

    if (node.kind === "project") {
      // Match group layout: Session section (including persistent browser/Resume), then project actions.
      return [
        ...newSessionItems(node.projectId, null, null, { withBrowser: true }),
        sep,
        {
          label: t("tree.newGroup"),
          onClick: () => openDialog({ type: "newGroup", projectId: node.projectId, parentGroupId: null }),
        },
        buildMarkItem("project", node.id),
        rename,
        {
          label: t("tree.removeProject"),
          danger: true,
          onClick: () => openDialog({ type: "confirmDelete", node, worktreePaths: [] }),
        },
      ];
    }
    if (node.kind === "group") {
      // A group's Git directory prefers its own worktree and falls back to the project root.
      const st = useTermStore.getState();
      const group = st.groups.find((g) => g.id === node.id);
      const gitRepoDir =
        group?.worktreePath ||
        (st.projects.find((p) => p.id === node.projectId)?.rootPath ?? "").trim() ||
        null;
      // Populate Git actions only for repository directories and pass the group ID to MergeModal. Omit
      // the entire section and separator when empty to avoid duplicate separators.
      const gitItems = buildGitItems({
        repoDir: gitRepoDir,
        worktreePath: group?.worktreePath,
        mergeTargetId: node.id,
      });
      // Group section contains child creation, worktree conversion/info, rename, archive, and delete.
      const groupBlock: MenuItem[] = [
        {
          label: t("tree.newSubgroup"),
          onClick: () => openDialog({ type: "newGroup", projectId: node.projectId, parentGroupId: node.id }),
        },
        // Convert to Regular Group appears only after the bound worktree directory is deleted. It clears
        // the binding; deleting a worktree and converting the group are separate operations.
        ...(isWorktreeGone(group?.worktreePath)
          ? [
              {
                label: t("tree.convertToNormalGroup"),
                onClick: () => void clearNodeWorktree("group", node.id),
              },
            ]
          : []),
        // Group Info is available only for worktree groups because regular groups have no relevant details.
        ...(group?.worktreePath
          ? [
              {
                label: t("tree.groupInfo"),
                onClick: () => openDialog({ type: "groupInfo", id: node.id }),
              },
            ]
          : []),
        buildMarkItem("group", node.id),
        rename,
        { label: t("tree.archiveGroup"), onClick: () => void archiveGroup(node.id) },
        {
          label: t("tree.deleteGroup"),
          danger: true,
          onClick: () => openDialog({ type: "confirmDelete", node, worktreePaths: [] }),
        },
      ];
      // Layout: Session, Git, then Group actions. Place Delete Group last to reduce accidental clicks.
      return [
        ...newSessionItems(node.projectId, node.id, null, { withBrowser: true, withTerminal: true }),
        ...(gitItems.length ? [sep, ...gitItems] : []),
        sep,
        ...groupBlock,
      ];
    }
    // Draft variants use a compact menu without delete/archive, offering persistence and close actions.
    if (node.id in ephemeralSessions) {
      return buildScratchItems(node, { variant: "terminal", onRename: () => startRename(node) });
    }
    if (node.id.startsWith("browser-") && browserTabs[node.id]) {
      return buildScratchItems(node, { variant: "browser", onRename: () => startRename(node) });
    }
    if (docTabs[node.id]?.isNew) {
      return buildScratchItems(node, { variant: "doc", onRename: () => startRename(node) });
    }
    // Session nodes reuse the shared single-session menu with sidebar inline onRename. Pass the pane the menu was
    // opened from so Refresh Status acts on that pane's own status filter.
    return buildSessionItems(node, {
      onRename: () => startRename(node),
      statusViewId: menu?.viewId ?? effectiveActiveViewId,
    });
  };

  return (
    <aside className="col col-left" style={{ width, borderRight: "none" }}>
      <div className="col-head">
        <span className="title">Workspace</span>
        <span className="sp" />
        <button
          className="icon-btn sm"
          title={t("tree.createProject")}
          onClick={() => useTermStore.getState().setCreateProjectModalOpen(true)}
        >
          <Icons.folderPlus size={14} />
        </button>
        <button className="icon-btn sm" title={t("tree.importProject")} onClick={() => void importProject()}>
          <Icons.folderOpen size={14} />
        </button>
        <button className="icon-btn sm" title={t("tree.cloneProject")} onClick={() => setCloneModalOpen(true)}>
          <Icons.git size={14} />
        </button>
        <button className="icon-btn sm" title={t("tree.globalSearch")} onClick={() => setGlobalSearchOpen(true)}>
          <Icons.search size={14} />
        </button>
        <button className="icon-btn sm" title={t("tree.archivedSessions")} onClick={() => setArchiveOpen(true)}>
          <Icons.archive size={14} />
        </button>
        {hasNotifications && (
          <button
            className="icon-btn sm"
            title={t("tree.clearAllNotifications")}
            onClick={clearAllNotifications}
          >
            <Icons.bellOff size={13} />
          </button>
        )}
      </div>

      <div ref={treeStageRef} className="sidebar-tree-stage">
        {activeLayout.map(({ leaf, rect }) => {
          const view = views.find((candidate) => candidate.id === leaf.viewId);
          return view ? (
            <div
              key={leaf.paneId}
              className="sidebar-tree-position"
              style={sidebarRectStyle(rect)}
            >
              <SidebarTreePane
                view={view}
                isPrimary={view.id === effectivePrimaryViewId}
                split={activeTabHasSplits}
                active={view.id === effectiveActiveViewId}
                onActivate={() => setActiveTreeView?.(view.id)}
                onSplitDown={() => splitTreeView?.("vertical", view.id)}
                onClose={() => deleteTreeView?.(view.id)}
                treeHandlers={{
                  onContext: (node, x, y) => {
                    setActiveTreeView?.(view.id);
                    setMenu({ node, viewId: view.id, x, y });
                  },
                  contextId: menu?.viewId === view.id ? menu.node.id : null,
                  renamingId: renaming?.viewId === view.id ? renaming.id : null,
                  renameVal,
                  setRenameVal,
                  commitRename,
                  cancelRename,
                  onAddSession,
                  onAddGroup,
                }}
              />
            </div>
          ) : null;
        })}
        {activeDividers.map((info) => (
          <SidebarTreeDivider
            key={info.paneId}
            tabId={activeTab.id}
            info={info}
            stageRef={treeStageRef}
          />
        ))}
      </div>

      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={buildMenu(menu.node)}
          onClose={() => setMenu(null)}
        />
      )}
      {newSessionMenu && (
        <ContextMenu
          x={newSessionMenu.x}
          y={newSessionMenu.y}
          items={newSessionMenu.items}
          onClose={() => setNewSessionMenu(null)}
        />
      )}

      {dialogs}

      {archiveOpen && <ArchivePanel />}
      {globalSearchOpen && <GlobalSearch />}
    </aside>
  );
}

function TreeScrollbar({ wrapRef }: { wrapRef: React.RefObject<HTMLDivElement | null> }) {
  const [thumb, setThumb] = useState<{ top: number; height: number } | null>(null);
  const [hovered, setHovered] = useState(false);
  const dragging = useRef(false);
  const trackRef = useRef<HTMLDivElement | null>(null);

  const update = useCallback(() => {
    const wrap = wrapRef.current;
    if (!wrap) return;
    const tree = wrap.querySelector<HTMLElement>(".tree");
    if (!tree) return;
    const { scrollTop, scrollHeight, clientHeight } = tree;
    if (scrollHeight <= clientHeight + 1) {
      setThumb(null);
      return;
    }
    const ratio = clientHeight / scrollHeight;
    const thumbH = Math.max(24, Math.round(ratio * clientHeight));
    const maxScroll = scrollHeight - clientHeight;
    const maxTop = clientHeight - thumbH;
    const t = maxScroll > 0 ? Math.round((scrollTop / maxScroll) * maxTop) : 0;
    setThumb({ top: t, height: thumbH });
  }, [wrapRef]);

  useEffect(() => {
    const wrap = wrapRef.current;
    if (!wrap) return;
    const tree = wrap.querySelector<HTMLElement>(".tree");
    if (!tree) return;

    let raf = 0;
    const onScroll = () => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(update);
    };

    tree.addEventListener("scroll", onScroll, { passive: true });
    const ro = new ResizeObserver(onScroll);
    ro.observe(tree);
    const mo = new MutationObserver(onScroll);
    mo.observe(tree, { childList: true, subtree: true });
    update();

    return () => {
      cancelAnimationFrame(raf);
      tree.removeEventListener("scroll", onScroll);
      ro.disconnect();
      mo.disconnect();
    };
  }, [wrapRef, update]);

  if (!thumb) return null;

  const scrollTo = (fraction: number) => {
    const tree = wrapRef.current?.querySelector<HTMLElement>(".tree");
    if (!tree) return;
    const clamped = Math.max(0, Math.min(1, fraction));
    tree.scrollTop = clamped * (tree.scrollHeight - tree.clientHeight);
  };

  const onTrackClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if (dragging.current) return;
    const rect = e.currentTarget.getBoundingClientRect();
    scrollTo((e.clientY - rect.top) / rect.height);
  };

  const onThumbMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragging.current = true;
    const track = trackRef.current;
    if (!track) return;
    const rect = track.getBoundingClientRect();
    const onMove = (me: MouseEvent) => {
      scrollTo((me.clientY - rect.top) / rect.height);
    };
    const onUp = () => {
      dragging.current = false;
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  };

  return (
    <div
      ref={trackRef}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => !dragging.current && setHovered(false)}
      onClick={onTrackClick}
      style={{
        position: "absolute",
        top: 4,
        right: 1,
        width: 7,
        bottom: 4,
        borderRadius: 4,
        background: hovered ? "var(--border-strong)" : "transparent",
        zIndex: 4,
        cursor: "pointer",
        transition: "background .15s",
      }}
    >
      <div
        onMouseDown={onThumbMouseDown}
        style={{
          position: "absolute",
          top: thumb.top,
          left: 0,
          right: 0,
          height: thumb.height,
          borderRadius: 4,
          background: hovered ? "var(--text-dim)" : "var(--text-faint)",
          transition: "background .15s",
        }}
      />
    </div>
  );
}
