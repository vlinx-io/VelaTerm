//! Vlinx-style center-pane tab bar. Each tab has a kind icon, title, close action, and active top accent.
//! The trailing + creates a scratch terminal; desktop clients also expose terminal-session and built-in
//! browser actions. Session tooltips show project/group/parent/session ancestry. Context actions vary:
//! persisted sessions combine close actions with shared sessionMenu operations, documents add refresh,
//! scratch terminals add inline renaming, and browser tabs expose close actions only. StatusBar lists
//! background liveTabs.

import { useCallback, useEffect, useRef, useState } from "react";
import { ContextMenu, type MenuItem } from "../../components/ContextMenu";
import Icons from "../../components/Icons";
import { StatusIndicator } from "../../components/StatusIndicator";
import { useT } from "../../i18n";
import { DOC_EXPORT_PDF_EVENT } from "../../hooks/useKeyboardShortcuts";
import { isTauri } from "../../ipc/transport";
import { env } from "../../platform";
import { labelWithCombo } from "../../hooks/shortcutRegistry";
import { useTermStore } from "../../store/termStore";
import { effectiveStatus, type Session } from "../../types";
import { type TreeNodeRef } from "../LeftSidebar/ProjectTree";
import { useSessionMenu } from "../sessionMenu";
import { SessionKindIcon } from "../sessionViewers/sessionMeta";

/** Close any tab, routing dirty documents through their three-choice confirmation and closing others directly. */
function closeAnyTab(tabId: string) {
  const st = useTermStore.getState();
  if (st.docTabs[tabId]) st.requestCloseDocTab(tabId);
  else st.closeTab(tabId);
}

/** Isolated session-status indicator subscribing only to this session's derived state and unread flag.
 * Agent signals rerender only the changed tab rather than the whole TabBar and its repeated lookups and
 * breadcrumbs. Unread follows ProjectTree notification semantics and clears after two seconds of viewing,
 * so activation does not immediately change Waiting for Reply to Seen. */
function SessionTabStatus({ tabId }: { tabId: string }) {
  const status = useTermStore((s) => effectiveStatus(s.runtimes[tabId]));
  const unread = useTermStore((s) => tabId in s.notifications);
  return <StatusIndicator status={status} unread={unread} />;
}

// Show a fast frontend tooltip before the native title while retaining the native fallback. Do not rely on
// mouseleave: WebKit omits it when tabs scroll/reflow beneath a stationary pointer. Global pointer bounds,
// scrolling, focus loss, and a four-second safety timeout dismiss any stranded tooltip.
const TIP_SHOW_DELAY = 120; // Must appear well before the native tooltip, typically after 500ms.
const TIP_MAX_VISIBLE = 4000; // Safety limit in milliseconds.

// Custom dataTransfer type identifies internal tab drags and rejects external ones.
const TAB_DRAG_MIME = "application/x-vlx-tab";

/** Match sidebar rename fields: reject control characters that WebKit can insert at input boundaries. */
const CTRL_CHARS_RE = /[\u0000-\u001F\u007F-\u009F]/g;
const stripControlChars = (s: string) => s.replace(CTRL_CHARS_RE, "");

function useCtrlCharGuard() {
  return useCallback((el: HTMLInputElement | null) => {
    if (!el) return;
    const onBeforeInput = (ev: Event) => {
      const data = (ev as InputEvent).data;
      if (typeof data === "string" && data !== stripControlChars(data)) ev.preventDefault();
    };
    el.addEventListener("beforeinput", onBeforeInput);
    return () => el.removeEventListener("beforeinput", onBeforeInput);
  }, []);
}

export function TabBar() {
  const t = useT();
  const shortcutOverrides = useTermStore((s) => s.shortcutOverrides);
  const tabRefs = useRef<Record<string, HTMLDivElement | null>>({});
  const ctrlCharGuard = useCtrlCharGuard();
  const projects = useTermStore((s) => s.projects);
  const groups = useTermStore((s) => s.groups);
  const sessions = useTermStore((s) => s.sessions);
  const ephemeralSessions = useTermStore((s) => s.ephemeralSessions);
  // Do not subscribe to complete runtime/notification maps; SessionTabStatus owns focused subscriptions.
  const openTabs = useTermStore((s) => s.openTabs);
  const activeTabId = useTermStore((s) => s.activeTabId);
  const docTabs = useTermStore((s) => s.docTabs);
  const browserTabs = useTermStore((s) => s.browserTabs);
  const setActiveTab = useTermStore((s) => s.setActiveTab);
  const closeTab = useTermStore((s) => s.closeTab);
  const requestCloseDocTab = useTermStore((s) => s.requestCloseDocTab);
  const refreshDocTab = useTermStore((s) => s.refreshDocTab);
  const newScratchTab = useTermStore((s) => s.newScratchTab);
  const newDocTab = useTermStore((s) => s.newDocTab);
  const moveTabToBackground = useTermStore((s) => s.moveTabToBackground);
  const reorderTab = useTermStore((s) => s.reorderTab);
  const renameScratch = useTermStore((s) => s.renameScratch);

  // Shared session operations, temporary-session actions, shell switching, and supporting dialogs.
  const { buildSessionItems, buildScratchItems, shellSwitchItems, dialogs } = useSessionMenu();

  // Tab context menu assembled by tab type during rendering.
  const [menu, setMenu] = useState<{ x: number; y: number; tabId: string } | null>(null);

  // Temporary terminals have no Edit Session dialog, so their context-menu Rename action edits the tab title
  // in place and persists it through the existing frontend-layout path.
  const [renamingTabId, setRenamingTabId] = useState<string | null>(null);
  const [renameVal, setRenameVal] = useState("");
  const startTabRename = (session: Session) => {
    setRenamingTabId(session.id);
    setRenameVal(session.name);
  };
  const commitTabRename = () => {
    const name = stripControlChars(renameVal).trim();
    if (renamingTabId && name) renameScratch(renamingTabId, name);
    setRenamingTabId(null);
  };

  // HTML5 tab reordering, matching ProjectTree. A custom dataTransfer type is visible during dragover even
  // when data is not, preventing text/plain ProjectTree session drags from crossing into the tab bar.
  const [dragTabId, setDragTabId] = useState<string | null>(null);
  // Draw a vertical insertion marker before or after the target tab.
  const [dropMark, setDropMark] = useState<{ id: string; side: "before" | "after" } | null>(null);

  const isTabDrag = (e: React.DragEvent) => e.dataTransfer.types.includes(TAB_DRAG_MIME);
  const calcSide = (e: React.DragEvent): "before" | "after" => {
    const r = e.currentTarget.getBoundingClientRect();
    return e.clientX < r.left + r.width / 2 ? "before" : "after";
  };
  const clearDrag = () => {
    setDragTabId(null);
    setDropMark(null);
  };

  /** Drag properties shared by all tab types and attached to their root div. */
  const dragProps = (tabId: string) => ({
    draggable: renamingTabId !== tabId,
    onDragStart: (e: React.DragEvent) => {
      e.dataTransfer.setData(TAB_DRAG_MIME, tabId);
      e.dataTransfer.effectAllowed = "move";
      setDragTabId(tabId);
      // Dragging suppresses mousemove, so dismiss the tooltip and pending timer as drag begins.
      onTabLeave();
    },
    onDragOver: (e: React.DragEvent) => {
      if (!isTabDrag(e)) return;
      e.preventDefault();
      e.stopPropagation(); // Container drops on empty space move to the end.
      e.dataTransfer.dropEffect = "move";
      if (tabId === dragTabId) return setDropMark(null); // No marker over the dragged tab itself.
      const side = calcSide(e);
      setDropMark((m) => (m?.id === tabId && m.side === side ? m : { id: tabId, side }));
    },
    onDragLeave: () => setDropMark((m) => (m?.id === tabId ? null : m)),
    onDrop: (e: React.DragEvent) => {
      if (!isTabDrag(e)) return;
      e.preventDefault();
      e.stopPropagation();
      const from = e.dataTransfer.getData(TAB_DRAG_MIME);
      if (from) reorderTab(from, tabId, calcSide(e));
      clearDrag();
    },
    onDragEnd: clearDrag, // Release and Escape cancellation share cleanup.
  });

  /** Tab classes for active state, dragging opacity, and insertion marker. */
  const tabClass = (tabId: string, isActive: boolean) =>
    "tab" +
    (isActive ? " on" : "") +
    (dragTabId === tabId ? " dragging" : "") +
    (dropMark?.id === tabId ? ` drop-${dropMark.side}` : "");

  // Dropping on tab-bar whitespace, including the action area, moves the tab to the end.
  const lastTabId = openTabs[openTabs.length - 1];
  const onBarDragOver = (e: React.DragEvent) => {
    if (!isTabDrag(e)) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    if (!lastTabId || lastTabId === dragTabId) return setDropMark(null);
    setDropMark((m) => (m?.id === lastTabId && m.side === "after" ? m : { id: lastTabId, side: "after" }));
  };
  const onBarDrop = (e: React.DragEvent) => {
    if (!isTabDrag(e)) return;
    e.preventDefault();
    const from = e.dataTransfer.getData(TAB_DRAG_MIME);
    if (from && lastTabId) reorderTab(from, lastTabId, "after");
    clearDrag();
  };

  const openTabMenu = (e: React.MouseEvent, tabId: string) => {
    e.preventDefault();
    setMenu({ x: e.clientX, y: e.clientY, tabId });
  };

  // Frontend tooltip text/screen coordinates; null hides it and showTimer controls its early delay.
  const [tip, setTip] = useState<{ text: string; x: number; y: number } | null>(null);
  const showTimer = useRef<number | null>(null);
  // Triggering tab node used by global dismissal to test current pointer bounds.
  const tipTabEl = useRef<HTMLElement | null>(null);

  const onTabEnter = (e: React.MouseEvent<HTMLElement>, text: string) => {
    if (!text) return;
    const el = e.currentTarget;
    const rect = el.getBoundingClientRect();
    // Anchor below the tab and reserve room for the 360px maximum width.
    const x = Math.max(8, Math.min(rect.left, window.innerWidth - 372));
    const y = rect.bottom + 4;
    if (showTimer.current != null) window.clearTimeout(showTimer.current);
    showTimer.current = window.setTimeout(() => {
      tipTabEl.current = el;
      setTip({ text, x, y });
    }, TIP_SHOW_DELAY);
  };
  const onTabLeave = () => {
    if (showTimer.current != null) {
      window.clearTimeout(showTimer.current);
      showTimer.current = null;
    }
    setTip(null);
  };

  // Global tooltip dismissal independent of unreliable mouseleave during tab reflow.
  useEffect(() => {
    if (!tip) return;
    const dismiss = () => setTip(null);
    // 1. Dismiss when global pointer movement leaves the trigger's current rectangle.
    const onMove = (ev: MouseEvent) => {
      const el = tipTabEl.current;
      if (!el) return dismiss();
      const r = el.getBoundingClientRect();
      const out =
        ev.clientX < r.left || ev.clientX > r.right || ev.clientY < r.top || ev.clientY > r.bottom;
      if (out) dismiss();
    };
    // 2. Any captured scroll may move tabs beneath a stationary pointer, so dismiss immediately.
    // 3. Window blur yields to another app or native tooltip. 4. Enforce TIP_MAX_VISIBLE as a safety cap.
    const safety = window.setTimeout(dismiss, TIP_MAX_VISIBLE);
    window.addEventListener("mousemove", onMove, true);
    window.addEventListener("scroll", dismiss, true);
    window.addEventListener("blur", dismiss);
    return () => {
      window.clearTimeout(safety);
      window.removeEventListener("mousemove", onMove, true);
      window.removeEventListener("scroll", dismiss, true);
      window.removeEventListener("blur", dismiss);
    };
  }, [tip]);

  // Clear any pending early-tooltip timer on unmount.
  useEffect(
    () => () => {
      if (showTimer.current != null) window.clearTimeout(showTimer.current);
    },
    [],
  );

  // Session tooltip breadcrumb: project, group chain, parent-session chain, and session name.
  const sessionPathTitle = (session: Session): string => {
    const parts: string[] = [session.name];
    // Walk spawned parent sessions upward, using seen to guard against cycles.
    const seen = new Set<string>([session.id]);
    let cur = session;
    while (cur.parentSessionId && !seen.has(cur.parentSessionId)) {
      const parent =
        sessions.find((s) => s.id === cur.parentSessionId) ??
        ephemeralSessions[cur.parentSessionId];
      if (!parent) break;
      seen.add(parent.id);
      parts.unshift(parent.name);
      cur = parent;
    }
    // Walk the top-level session's group chain back to the project root.
    let gid = cur.groupId ?? null;
    while (gid && !seen.has(gid)) {
      seen.add(gid);
      const g = groups.find((x) => x.id === gid);
      if (!g) break;
      parts.unshift(g.name);
      gid = g.parentGroupId ?? null;
    }
    const project = projects.find((p) => p.id === session.projectId);
    if (project) parts.unshift(project.name);
    return parts.join(" › ");
  };

  useEffect(() => {
    if (activeTabId) {
      tabRefs.current[activeTabId]?.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
    }
  }, [activeTabId]);

  // Tab creation, closure, or activation can reflow tabs without pointer movement or scrolling. Clear any
  // tooltip and pending timer when the tab set or active tab changes.
  useEffect(() => {
    if (showTimer.current != null) {
      window.clearTimeout(showTimer.current);
      showTimer.current = null;
    }
    setTip(null);
  }, [openTabs, activeTabId]);

  return (
    <div className="tabbar" onDragOver={onBarDragOver} onDrop={onBarDrop}>
      {openTabs.map((tabId) => {
        // Document tab: file icon, filename, dirty marker, and close action.
        const doc = docTabs[tabId];
        if (doc) {
          const isActive = tabId === activeTabId;
          return (
            <div
              key={tabId}
              ref={(el) => { tabRefs.current[tabId] = el; }}
              className={tabClass(tabId, isActive)}
              title={doc.path}
              onClick={() => setActiveTab(tabId)}
              onContextMenu={(e) => openTabMenu(e, tabId)}
              onMouseEnter={(e) => onTabEnter(e, doc.path)}
              onMouseLeave={onTabLeave}
              {...dragProps(tabId)}
            >
              <span style={{ color: "var(--text-dim)", display: "grid", flex: "none" }}>
                {doc.kind === "image" ? <Icons.image size={13} /> : <Icons.file size={13} />}
              </span>
              <span className="tnm">{doc.title}</span>
              {doc.dirty && (
                <span
                  title={t("tab.unsavedDot")}
                  style={{
                    width: 7,
                    height: 7,
                    borderRadius: "50%",
                    background: "var(--accent)",
                    flex: "none",
                  }}
                />
              )}
              <span
                className="x"
                onClick={(e) => {
                  e.stopPropagation();
                  // Route through dirty confirmation rather than closing directly.
                  requestCloseDocTab(tabId);
                }}
              >
                <Icons.x size={12} />
              </span>
            </div>
          );
        }
        // Built-in browser tab: globe, host or blank-page placeholder, and close action.
        const browser = browserTabs[tabId];
        if (browser) {
          const isActive = tabId === activeTabId;
          const label = browser.title || (browser.url === "about:blank" ? t("tab.newBrowserTab") : browser.url);
          return (
            <div
              key={tabId}
              ref={(el) => { tabRefs.current[tabId] = el; }}
              className={tabClass(tabId, isActive)}
              title={browser.url}
              onClick={() => setActiveTab(tabId)}
              onContextMenu={(e) => openTabMenu(e, tabId)}
              onMouseEnter={(e) => onTabEnter(e, browser.url)}
              onMouseLeave={onTabLeave}
              {...dragProps(tabId)}
            >
              <span style={{ color: "var(--text-dim)", display: "grid", flex: "none" }}>
                <Icons.globe size={13} />
              </span>
              <span className="tnm">{label}</span>
              {browser.loading && (
                <span
                  title={t("browser.loading")}
                  style={{
                    width: 7,
                    height: 7,
                    borderRadius: "50%",
                    background: "var(--text-dim)",
                    flex: "none",
                  }}
                />
              )}
              <span
                className="x"
                onClick={(e) => {
                  e.stopPropagation();
                  // Web pages have no unsaved guard; close directly and destroy the child WebView.
                  closeTab(tabId);
                }}
              >
                <Icons.x size={12} />
              </span>
            </div>
          );
        }
        const session = sessions.find((s) => s.id === tabId) ?? ephemeralSessions[tabId];
        if (!session) return null;
        const isAgent = session.kind !== "terminal";
        const isActive = tabId === activeTabId;
        // SessionTabStatus owns status/unread subscriptions. Compute one breadcrumb for both tooltip paths.
        const pathTitle = sessionPathTitle(session);
        return (
          <div
            key={tabId}
            ref={(el) => { tabRefs.current[tabId] = el; }}
            className={tabClass(tabId, isActive)}
            title={pathTitle}
            onClick={() => setActiveTab(tabId)}
            onContextMenu={(e) => openTabMenu(e, tabId)}
            onMouseEnter={(e) => onTabEnter(e, pathTitle)}
            onMouseLeave={onTabLeave}
            {...dragProps(tabId)}
          >
            <span
              style={{
                color: isAgent ? "var(--text-secondary)" : "var(--text-dim)",
                display: "grid",
                flex: "none",
                position: "relative",
              }}
            >
              <SessionKindIcon
                session={session}
                size={13}
                rootPath={projects.find((pr) => pr.id === session.projectId)?.rootPath ?? null}
              />
            </span>
            {renamingTabId === tabId ? (
              <input
                ref={ctrlCharGuard}
                className="rename-input"
                autoFocus
                value={renameVal}
                onClick={(e) => e.stopPropagation()}
                onChange={(e) => setRenameVal(stripControlChars(e.target.value))}
                onKeyDown={(e) => {
                  if (e.key === "Enter") commitTabRename();
                  if (e.key === "Escape") setRenamingTabId(null);
                }}
                onBlur={commitTabRename}
              />
            ) : (
              <span className="tnm">{session.name}</span>
            )}
            <SessionTabStatus tabId={tabId} />
            <span
              className="x"
              onClick={(e) => {
                e.stopPropagation();
                closeTab(tabId);
              }}
            >
              <Icons.x size={12} />
            </span>
          </div>
        );
      })}
      <button className="tab-add" title={t("tab.newTerminal")} onClick={() => newScratchTab()}>
        <Icons.plus size={14} />
      </button>
      {/* Permanent entries on the right, separated from the tabs: new terminal session, new document, built-in browser tab. */}
      <div style={{ marginLeft: "auto", display: "flex", flex: "none" }}>
        <button className="tab-add" title={t("tab.newTerminal")} onClick={() => newScratchTab()}>
          <Icons.terminal size={14} />
        </button>
        {/* New document, available in every shell. The desktop uses the native Save As dialog to write
            the file; a plain browser or remote window has no native dialog, so saving a draft opens the
            server-side path picker instead (store.promptSaveAs → SaveAsModal); see DocView.save. */}
        <button className="tab-add" title={t("tab.newDocument")} onClick={newDocTab}>
          <Icons.filePlus size={14} />
        </button>
        {(isTauri || env.isElectron) && (
          <button
            className="tab-add"
            title={labelWithCombo(t("titlebar.browser"), "newBrowserTab", shortcutOverrides)}
            onClick={() => useTermStore.getState().openBrowserTab()}
          >
            <Icons.compass size={14} />
          </button>
        )}
      </div>
      {tip && (
        // Early tooltip uses fixed positioning, ignores pointer events, and uses theme raised-panel color.
        <div
          style={{
            position: "fixed",
            left: tip.x,
            top: tip.y,
            zIndex: 9999,
            maxWidth: 360,
            padding: "6px 10px",
            borderRadius: 6,
            background: "var(--bg-2)",
            color: "var(--text)",
            border: "1px solid var(--border)",
            boxShadow: "0 6px 20px rgba(0,0,0,0.35)",
            fontSize: 12,
            lineHeight: 1.5,
            wordBreak: "break-word",
            pointerEvents: "none",
          }}
        >
          {tip.text}
        </div>
      )}
      {menu &&
        (() => {
          const tabId = menu.tabId;
          const idx = openTabs.indexOf(tabId);
          // Close actions shared by all four tab types.
          const closeItems: MenuItem[] = [
            { label: t("doc.closeTab"), onClick: () => closeAnyTab(tabId) },
            {
              label: t("tab.closeOthers"),
              disabled: openTabs.length <= 1,
              onClick: () => {
                for (const id of openTabs) if (id !== tabId) closeAnyTab(id);
              },
            },
            {
              label: t("tab.closeRight"),
              disabled: idx < 0 || idx === openTabs.length - 1,
              onClick: () => {
                for (const id of openTabs.slice(idx + 1)) closeAnyTab(id);
              },
            },
            {
              label: t("tab.closeAll"),
              onClick: () => {
                for (const id of openTabs) closeAnyTab(id);
              },
            },
          ];

          let items: MenuItem[];
          if (docTabs[tabId]) {
            // Draft documents add Save to Disk; persisted documents add Refresh. Both offer Markdown PDF
            // export and common close actions.
            const doc = docTabs[tabId];
            const scratchNode: TreeNodeRef = {
              kind: "session",
              id: tabId,
              name: doc?.title ?? "",
              projectId: "",
              groupId: null,
            };
            const headItems: MenuItem[] = doc?.isNew
              ? buildScratchItems(scratchNode, { variant: "doc", showOpen: false, omitClose: true })
              : [{ label: t("tab.refreshFile"), onClick: () => refreshDocTab(tabId) }];
            const mdItems: MenuItem[] =
              doc?.kind === "markdown"
                ? [
                    {
                      label: t("doc.exportPdf"),
                      onClick: () =>
                        window.dispatchEvent(
                          new CustomEvent(DOC_EXPORT_PDF_EVENT, { detail: tabId }),
                        ),
                    },
                  ]
                : [];
            items = [...headItems, ...mdItems, { label: "", separator: true }, ...closeItems];
          } else {
            // Persisted sessions receive full operations; draft terminals/browsers receive temporary actions
            // such as conversion; other browser tabs retain only close actions.
            const rec = sessions.find((s) => s.id === tabId);
            const ephRec = ephemeralSessions[tabId];
            const browserDraft = tabId.startsWith("browser-") ? browserTabs[tabId] : undefined;
            if (rec) {
              const node: TreeNodeRef = {
                kind: "session",
                id: rec.id,
                name: rec.name,
                projectId: rec.projectId,
                groupId: rec.groupId ?? null,
              };
              // Omit onRename because tabs use Edit rather than inline renaming. Move to Background is a
              // tab-level alternative to close that keeps the pane tree alive, available only to session tabs.
              items = [
                ...closeItems,
                { label: "", separator: true },
                { label: t("tab.sendToBackground"), onClick: () => moveTabToBackground(tabId) },
                { label: "", separator: true },
                ...buildSessionItems(node),
              ];
            } else if (ephRec || browserDraft) {
              const node: TreeNodeRef = {
                kind: "session",
                id: tabId,
                name: ephRec ? ephRec.name : browserDraft!.title,
                projectId: ephRec?.projectId ?? "",
                groupId: ephRec?.groupId ?? null,
              };
              // Common close actions already exist, so add only conversion and information to temporary menus.
              items = [
                ...closeItems,
                { label: "", separator: true },
                ...buildScratchItems(node, {
                  variant: ephRec ? "terminal" : "browser",
                  showOpen: false,
                  omitClose: true,
                  onRename: ephRec ? () => startTabRename(ephRec) : undefined,
                }),
              ];
            } else {
              // Ephemeral terminals add Shell quick switching; nonterminal tabs receive an empty list.
              const shellItems = shellSwitchItems(tabId);
              items = shellItems.length
                ? [...closeItems, { label: "", separator: true }, ...shellItems]
                : closeItems;
            }
          }
          return <ContextMenu x={menu.x} y={menu.y} items={items} onClose={() => setMenu(null)} />;
        })()}
      {dialogs}
    </div>
  );
}
