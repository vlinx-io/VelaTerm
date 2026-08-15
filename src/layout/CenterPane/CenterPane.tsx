//! Vlinx-style center pane: a tab bar plus a terminal area with recursive splits and draggable dividers.
//! Every session pane remains mounted in an absolutely positioned percentage-based rectangle.
//! Inactive tabs use `display:none`, keeping xterm and the PTY alive; dividers overlay the active tab.

import { lazy, Suspense, useCallback, useMemo, useRef } from "react";
import Icons from "../../components/Icons";
import { useT } from "../../i18n";
import { isMac } from "../../ipc/transport";
import { useTermStore } from "../../store/termStore";
import {
  collectSessionIds,
  computeDividers,
  computeLayout,
  type DividerInfo,
  type Rect,
} from "./paneTree";
import { BrowserView } from "./browser/BrowserView";
import { DormantPane } from "./DormantPane";
import { LiveTabsOverLimitDialog } from "./LiveTabsOverLimitDialog";
import { SearchBar } from "./SearchBar";
import { TabBar } from "./TabBar";
import { TerminalView } from "./TerminalView";

// Dynamically import the entire document editor. Crepe/CodeMirror plus ProseMirror exceeds 1 MB
// before compression, so Vite splits it into a chunk that does not affect terminal startup.
const DocView = lazy(() =>
  import("./doc/DocView").then((m) => ({ default: m.DocView })),
);

const rectToStyle = (r: Rect): React.CSSProperties => ({
  left: `${r.left}%`,
  top: `${r.top}%`,
  width: `${r.width}%`,
  height: `${r.height}%`,
});

const FULL: React.CSSProperties = { left: 0, top: 0, width: "100%", height: "100%" };

/** Draggable divider that converts pixel movement within its container into percentage deltas. */
function Divider({
  info,
  tabId,
  stageRef,
}: {
  info: DividerInfo;
  tabId: string;
  stageRef: React.RefObject<HTMLDivElement | null>;
}) {
  const resizePane = useTermStore((s) => s.resizePane);
  const horiz = info.dir === "horizontal";

  const startDrag = (e: React.MouseEvent) => {
    e.preventDefault();
    const stage = stageRef.current;
    if (!stage) return;
    const rect = stage.getBoundingClientRect();
    const parentPx = horiz
      ? (rect.width * info.parentRect.width) / 100
      : (rect.height * info.parentRect.height) / 100;
    if (parentPx <= 0) return;
    const startPos = horiz ? e.clientX : e.clientY;
    const [a0, b0] = info.sizes;
    const el = e.currentTarget as HTMLElement;
    el.classList.add("dragging");

    const move = (ev: MouseEvent) => {
      const pos = horiz ? ev.clientX : ev.clientY;
      let d = ((pos - startPos) / parentPx) * 100;
      d = Math.max(-(a0 - 10), Math.min(b0 - 10, d)); // Keep at least 10% on each side.
      resizePane(tabId, info.paneId, [a0 + d, b0 - d]);
    };
    const up = () => {
      el.classList.remove("dragging");
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
      document.body.style.userSelect = "";
    };
    document.body.style.userSelect = "none";
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
  };

  const style: React.CSSProperties = horiz
    ? {
        position: "absolute",
        left: `${info.leftPct}%`,
        top: `${info.topPct}%`,
        height: `${info.lengthPct}%`,
        transform: "translateX(-50%)",
        zIndex: 6,
      }
    : {
        position: "absolute",
        left: `${info.leftPct}%`,
        top: `${info.topPct}%`,
        width: `${info.lengthPct}%`,
        transform: "translateY(-50%)",
        zIndex: 6,
      };

  return (
    <div
      className={"divider " + (horiz ? "dv-row" : "dv-col")}
      style={style}
      onMouseDown={startDrag}
    />
  );
}

export function CenterPane() {
  const t = useT();
  const projects = useTermStore((s) => s.projects);
  const sessions = useTermStore((s) => s.sessions);
  const ephemeralSessions = useTermStore((s) => s.ephemeralSessions);
  const openTabs = useTermStore((s) => s.openTabs);
  const liveTabs = useTermStore((s) => s.liveTabs);
  const docTabs = useTermStore((s) => s.docTabs);
  const browserTabs = useTermStore((s) => s.browserTabs);
  const paneTrees = useTermStore((s) => s.paneTrees);
  const activeTabId = useTermStore((s) => s.activeTabId);
  const activeSessionId = useTermStore((s) => s.activeSessionId);
  const epochs = useTermStore((s) => s.epochs);
  const dormantSessions = useTermStore((s) => s.dormantSessions);
  const searchOpen = useTermStore((s) => s.searchOpen);
  const focusPane = useTermStore((s) => s.focusPane);
  const closePane = useTermStore((s) => s.closePane);
  const splitNew = useTermStore((s) => s.splitNew);
  const newScratchTab = useTermStore((s) => s.newScratchTab);

  // Index sessions and projects by ID for O(1) lookup. Calling find for every entry in allIds.map
  // would make each tab, split, or session update O(mounted panes x sessions).
  const sessionsById = useMemo(() => new Map(sessions.map((s) => [s.id, s])), [sessions]);
  const projectsById = useMemo(() => new Map(projects.map((p) => [p.id, p])), [projects]);

  const stageRef = useRef<HTMLDivElement>(null);

  // Pass stable store-action references to each memoized TerminalView. Switching sessions then
  // renders only the old and new terminals; background terminals retain stable props. Pass paneId
  // as a prop so callbacks do not capture a newly created info object.
  const handleActivate = useCallback(
    (paneId: string, id: string) => focusPane(paneId, id),
    [focusPane],
  );
  const handleSplit = useCallback(
    (paneId: string, id: string, dir: "horizontal" | "vertical") => {
      focusPane(paneId, id);
      void splitNew(dir);
    },
    [focusPane, splitNew],
  );
  const handleClose = useCallback(
    (paneId: string, id: string) => {
      focusPane(paneId, id);
      closePane();
    },
    [focusPane, closePane],
  );

  // Active-tab layout: sessionId -> { rect, paneId }, plus dividers.
  const activeTree = activeTabId ? paneTrees[activeTabId] : null;
  const layout = activeTree ? computeLayout(activeTree) : [];
  const dividers = activeTree ? computeDividers(activeTree) : [];
  const visibleBySession = new Map(
    layout.map(({ leaf, rect }) => [leaf.sessionId, { rect, paneId: leaf.paneId }]),
  );
  const multi = layout.length > 1;

  // Collect sessions from visible and background keep-alive tabs. All remain mounted; hidden ones use display:none.
  const allIds = new Set<string>();
  for (const tabId of [...openTabs, ...liveTabs]) {
    const t = paneTrees[tabId];
    if (t) for (const sid of collectSessionIds(t)) allIds.add(sid);
  }

  return (
    <div className="col col-mid">
      <TabBar />
      <div className="stage" ref={stageRef} style={{ position: "relative" }}>
        {searchOpen && activeSessionId && <SearchBar />}

        {openTabs.length === 0 && (
          <div className="empty">
            <div className="inner">
              <div className="glyph">
                <Icons.terminal size={34} />
              </div>
              <div>{t("center.noSession")}</div>
              <div style={{ color: "var(--text-faint)" }}>
                {t("center.noSessionHintPre")}
                <kbd>{isMac ? "⌘T" : "Ctrl Alt T"}</kbd>
                {t("center.noSessionHintPost")}
              </div>
              <button className="empty-action" onClick={() => void newScratchTab()}>
                <Icons.terminal size={14} />
                {t("center.createTerminal")}
              </button>
            </div>
          </div>
        )}

        {[...allIds].map((id) => {
          const session = sessionsById.get(id) ?? ephemeralSessions[id];
          if (!session) return null;
          const project = projectsById.get(session.projectId);
          const cwd = session.cwd ?? project?.rootPath ?? undefined;
          const epoch = epochs[id] ?? 0;
          const info = visibleBySession.get(id);
          const visible = !!info;
          // A dormant leaf must not mount TerminalView, because mounting spawns the process.
          if (dormantSessions[id]) {
            return (
              <DormantPane
                key={`${id}:${epoch}`}
                session={session}
                area={info ? rectToStyle(info.rect) : FULL}
                hidden={!visible}
                onActivate={info ? () => handleActivate(info.paneId, id) : undefined}
              />
            );
          }
          return (
            <TerminalView
              key={`${id}:${epoch}`}
              session={session}
              cwd={cwd}
              area={info ? rectToStyle(info.rect) : FULL}
              hidden={!visible}
              focused={visible && id === activeSessionId}
              multi={multi}
              paneId={info?.paneId}
              onActivate={handleActivate}
              onSplit={handleSplit}
              onClose={handleClose}
            />
          );
        })}

        {/* Document tabs follow the terminal keep-alive model: every open view remains mounted and
            fills the stage. Inactive views use display:none, preserving edits, scroll position,
            and undo history until closeTab unmounts them. */}
        {openTabs
          .filter((tabId) => docTabs[tabId])
          .map((tabId) => (
            <Suspense key={tabId} fallback={null}>
              <DocView tab={docTabs[tabId]} hidden={tabId !== activeTabId} />
            </Suspense>
          ))}

        {/* Browser tabs also remain mounted because unmounting destroys the native child WebView.
            When inactive, BrowserView hides the child WebView while its process stays alive, and
            the toolbar uses display:none. */}
        {openTabs
          .filter((tabId) => browserTabs[tabId])
          .map((tabId) => (
            <BrowserView key={tabId} tab={browserTabs[tabId]} hidden={tabId !== activeTabId} />
          ))}

        {activeTabId &&
          dividers.map((d) => (
            <Divider key={d.paneId} info={d} tabId={activeTabId} stageRef={stageRef} />
          ))}
      </div>
      <LiveTabsOverLimitDialog />
    </div>
  );
}
