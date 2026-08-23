//! A single session pane in the Vlinx UI: pane header (status, title, branch, split, close) plus the real xterm.
//! Active panes are shown while background panes remain alive under display:none, preserving xterm and PTY.
//! `area` positions the pane within the terminal region as a percentage rectangle.

import { memo, useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import Icons from "../../components/Icons";
import { ContextMenu, type MenuItem } from "../../components/ContextMenu";
import { StatusIndicator } from "../../components/StatusIndicator";
import { kindIconEl } from "../sessionViewers/sessionMeta";
import { useT } from "../../i18n";
import { usePtySession } from "../../hooks/usePtySession";
import { useGitBranch } from "../../hooks/useGitBranch";
import { IS_PLAIN_BROWSER } from "../../hooks/shortcutRegistry";
import { copyText } from "../../ipc/info";
import {
  agentInstallRecipe,
  agentLocateBin,
  ptyWrite,
  type AgentInstallRecipe,
} from "../../ipc/commands";
import { flushNow as flushSettingsSync } from "../../ipc/settingsSync";
import { isMac } from "../../ipc/transport";
import { platform } from "../../platform";
import { env } from "../../platform/env";
import {
  imagesFromDrop,
  imageFromNativeClipboard,
  injectImageFiles,
  injectNativeImagePaste,
  planImagePaste,
  supportsNativeImagePaste,
} from "../../terminal/imageInput";
import { shellDisplayName, useTermStore } from "../../store/termStore";
import {
  clearTerminal,
  focusTerminal,
  getSelection,
  hasSelection,
  pasteToTerminal,
  recentOsc52Copy,
  redrawTerminal,
  selectAll,
} from "../../terminal/registry";
import { effectiveStatus, type Session } from "../../types";

// Platform-specific terminal search hint: Cmd+F on macOS shells, Ctrl+Alt+F on Windows/Linux and
// plain browsers (see IS_PLAIN_BROWSER / useKeyboardShortcuts).
const SEARCH_KEY = isMac && !IS_PLAIN_BROWSER ? "⌘F" : "Ctrl+Alt+F";

export const TerminalView = memo(function TerminalView({
  session,
  cwd,
  area,
  hidden,
  focused,
  multi,
  paneId,
  onActivate,
  onSplit,
  onClose,
}: {
  session: Session;
  cwd?: string;
  area: React.CSSProperties;
  hidden: boolean;
  focused: boolean;
  multi: boolean;
  /** ID of this session's current pane; defined only while visible. */
  paneId?: string;
  onActivate: (paneId: string, id: string) => void;
  onSplit: (paneId: string, id: string, dir: "horizontal" | "vertical") => void;
  onClose: (paneId: string, id: string) => void;
}) {
  const t = useT();
  const { containerRef, starting, sizeMode, ptyDims, takeoverSize } =
    usePtySession(session, cwd, hidden);
  const paneStyle = useTermStore((s) => s.paneStyle);
  const openSearch = useTermStore((s) => s.openSearch);
  // Agent-default image paste applies only to local desktop (Tauri/Electron). Browser and remote agents do not
  // share the clipboard machine, so they always upload. See Terminal > Image Paste and the design document.
  const imagePasteMode = useTermStore((s) => s.imagePasteMode);
  // Subscribe only to whether this is the current session. With memoization, switching rerenders only the
  // terminal being left and the terminal being entered; other mounted background terminals remain untouched.
  const isActive = useTermStore((s) => s.activeSessionId === session.id);
  // Subscribe to the derived status string rather than the runtime object. Every set changes object identity
  // and would rerender TerminalView even without a value change; Object.is naturally deduplicates strings.
  const status = useTermStore((s) => effectiveStatus(s.runtimes[session.id]));
  // Unread notifications follow Tab and project-tree semantics and clear after two seconds of viewing.
  const unread = useTermStore((s) => session.id in s.notifications);
  const isAgent = session.kind !== "terminal";
  const branch = useGitBranch(cwd ?? null);
  // Poll for the installation path during one-click install. Keep this on TerminalView because the card unmounts when hidden.
  useAgentInstallLocator(session);

  // Give xterm keyboard focus when this session becomes active. Switching sessions changes only store state
  // and visibility, not DOM focus; without this, keystrokes remain in the sidebar or previous control until the
  // terminal is clicked. Use visible + active session rather than relying on the header-highlight `focused`
  // prop, and wait one animation frame for display:block before focusing.
  // Skip that focus when this activation came from mirror mode rather than from this window: a peer
  // switching tabs must rearrange what is shown here without pulling the keyboard away from whoever is
  // typing locally. Clicking a terminal still focuses it — xterm handles that itself, no effect involved.
  // The marker is consumed here, so the next activation of this same session — one the user made — focuses
  // normally even while the peer keeps publishing frames.
  useEffect(() => {
    if (hidden || !isActive) return;
    if (useTermStore.getState().mirrorFocusSessionId === session.id) {
      useTermStore.setState({ mirrorFocusSessionId: null });
      return;
    }
    const raf = requestAnimationFrame(() => focusTerminal(session.id));
    return () => cancelAnimationFrame(raf);
  }, [hidden, isActive, session.id]);

  // Custom terminal context menu replacing the WebView default.
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);

  // Auto-dismissing banner for image-upload failures; console-only failures were too easy to miss.
  const [imgError, setImgError] = useState<string | null>(null);
  const imgErrorTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const showImageError = (message: string) => {
    setImgError(message);
    if (imgErrorTimer.current) clearTimeout(imgErrorTimer.current);
    imgErrorTimer.current = setTimeout(() => setImgError(null), 5000);
  };

  // Route pasted and dropped images through one upload-and-write-path flow, showing the banner on failure.
  const injectImages = (files: File[]) => {
    void injectImageFiles(session.id, files, session.kind).then((r) => {
      if (!r.fail) return;
      showImageError(t("term.imgUploadFailed", r.fail, r.lastError ?? ""));
    });
  };

  const injectNativeClipboardImage = () => {
    void imageFromNativeClipboard()
      .then((file) => injectImages([file]))
      .catch(() => showImageError(t("term.imgClipboardUnavailable")));
  };

  const stop = (e: React.SyntheticEvent) => e.stopPropagation();

  const onContextMenu = (e: React.MouseEvent) => {
    e.preventDefault(); // Suppress the system/browser context menu.
    setMenu({ x: e.clientX, y: e.clientY });
  };

  // Copy has three states: copy a normal xterm selection; if there is no selection but this session recently
  // copied through OSC 52 (TUIs such as Claude/Codex manage selection themselves), explain that N characters
  // were already copied; otherwise disable the item.
  const selectedText = hasSelection(session.id) ? getSelection(session.id) : "";
  const autoCopied = selectedText ? null : recentOsc52Copy(session.id);
  const copyItem: MenuItem = selectedText
    ? { label: t("common.copy"), onClick: () => void copyText(selectedText) }
    : autoCopied != null
      ? { label: t("term.autoCopied", autoCopied), disabled: true }
      : { label: t("common.copy"), disabled: true };

  const canReadClipboard =
    env.hasNativeHost || typeof navigator.clipboard?.readText === "function";

  /**
   * A context-menu paste has no browser paste event from which to obtain a File. On Tauri, inspect the native
   * clipboard for an image first and reuse the same upload/native routing as the keyboard shortcut; otherwise
   * paste text normally. Electron cannot read images yet, but native mode can still send Ctrl+V to a local agent.
   */
  const pasteFromContextMenu = async () => {
    try {
      let image: File | null = null;
      if (env.isTauri) {
        try {
          image = await imageFromNativeClipboard();
        } catch {
          // Fall through to text when the clipboard does not contain an image.
        }
      }

      if (image) {
        if (imagePasteMode === "agent" && supportsNativeImagePaste(session.kind)) {
          await injectNativeImagePaste(session.id);
        } else {
          injectImages([image]);
        }
        return;
      }

      const text = await platform.clipboard.readText();
      if (text) {
        pasteToTerminal(session.id, text);
        return;
      }

      if (
        imagePasteMode === "agent" &&
        (env.isTauri || env.isElectron) &&
        supportsNativeImagePaste(session.kind)
      ) {
        await injectNativeImagePaste(session.id);
      }
    } catch {
      showImageError(t("term.imgClipboardUnavailable"));
    } finally {
      focusTerminal(session.id);
    }
  };

  const menuItems: MenuItem[] = [
    copyItem,
    {
      // Plain HTTP remote access forbids asynchronous clipboard reads, so menu paste cannot work. Native xterm
      // paste events from Cmd+V still do, so disable the menu item and direct the user to the shortcut.
      label:
        canReadClipboard ? t("term.paste") : t("term.pasteUseShortcut"),
      disabled: !canReadClipboard,
      onClick: pasteFromContextMenu,
    },
    {
      label: t("term.selectAll"),
      onClick: () => selectAll(session.id),
    },
    { label: "", separator: true },
    {
      label: t("term.clear"),
      onClick: () => {
        clearTerminal(session.id);
        focusTerminal(session.id);
      },
    },
    {
      label: `${t("term.searchMenu")}  ${SEARCH_KEY}`,
      onClick: () => openSearch(),
    },
  ];

  return (
    <div
      className="term-mount"
      onMouseDown={() => {
        if (paneId) onActivate(paneId, session.id);
      }}
      style={{
        position: "absolute",
        ...area,
        display: hidden ? "none" : "block",
        // Card mode leaves gaps between panes; flush mode uses only a 1 px divider.
        padding: paneStyle === "card" ? "calc(var(--pane-gap) / 2)" : 0,
      }}
    >
      <div
        className={"pane" + (focused ? " focus" : "")}
        // contain:layout confines the display:none -> block reflow to this pane subtree instead of the whole
        // terminal region, reducing switch latency. Apply it to inner .pane rather than .term-mount: ContextMenu
        // is a non-portaled, fixed-position sibling, and containment on .term-mount would change its coordinate
        // basis from viewport to pane. .pane already contains the header and full xterm subtree, capturing the
        // expensive reflow without disturbing menu placement. Layout containment does not clip, and existing
        // absolutely positioned overlays are already bounded by .pane's overflow:hidden.
        style={{ width: "100%", height: "100%", contain: "layout" }}
      >
        {/* The pane header is always shown. Even a single, unsplit pane keeps it so the split button
            stays clickable and the user can split at any time; the close button is disabled while there
            is only one pane. The split shortcuts (⌘D/⌘⇧D, or Ctrl+Alt+D/E elsewhere) split as well. */}
        <div className="pane-head">
          {/* The kind icon and status dot reuse the same shared components as the tabs and the sidebar tree, keeping all three consistent. */}
          <span
            style={{
              color: isAgent ? "var(--text-secondary)" : "var(--text-dim)",
              display: "grid",
              flex: "none",
            }}
          >
            {kindIconEl(session.kind, 13)}
          </span>
          <span className="pt">{session.name}</span>
          <StatusIndicator status={status} unread={unread} />
          {branch && (
            <span className="branch">
              <Icons.branch size={11} />
              {branch}
            </span>
          )}
          {/* Terminal sessions put the shell picker in the header, left of the tool buttons, rather than over the bottom-right of xterm; see ShellPicker. */}
          <ShellPicker session={session} />
          <span className="pane-tools">
            <button
              title={t("term.redraw")}
              onMouseDown={stop}
              onClick={(e) => {
                stop(e);
                redrawTerminal(session.id);
                focusTerminal(session.id);
              }}
            >
              <Icons.restart size={14} />
            </button>
            <button
              title={t("term.splitRight")}
              onMouseDown={stop}
              onClick={(e) => {
                stop(e);
                if (paneId) onSplit(paneId, session.id, "horizontal");
              }}
            >
              <Icons.splitV size={14} />
            </button>
            <button
              title={t("term.splitDown")}
              onMouseDown={stop}
              onClick={(e) => {
                stop(e);
                if (paneId) onSplit(paneId, session.id, "vertical");
              }}
            >
              <Icons.splitH size={14} />
            </button>
            <button
              title={t("term.closePane")}
              disabled={!multi}
              onMouseDown={stop}
              onClick={(e) => {
                stop(e);
                if (multi && paneId) onClose(paneId, session.id);
              }}
            >
              <Icons.close size={14} />
            </button>
          </span>
        </div>

        {/* The xterm container fills its parent through absolute positioning so it has a definite size.
            It deliberately does not rely on height:100% inside a flex parent: WebKit resolves that
            percentage height unreliably, fit() then measures the wrong number of rows, and the Claude
            TUI repaints against that wrong count, overwriting its final answer or pushing it out of
            view. */}
        <div
          style={{ flex: 1, minHeight: 0, position: "relative" }}
          onContextMenu={onContextMenu}
          // When right-clicking an existing selection, intercept both mousedown and mouseup during capture,
          // before xterm listeners. Otherwise a mouse-reporting TUI receives both events through CoreMouseService
          // as user input, causing SelectionService to clear the selection before the menu opens and leaving Copy disabled.
          //
          // Intercept mouseup as well: WebView2 fires contextmenu after mouseup, so blocking only mousedown still
          // lets the release clear the selection before the menu appears. macOS fires contextmenu on mousedown,
          // which hid this bug. Swallowing both also avoids sending an unmatched release. With no selection,
          // both events continue to the application normally.
          onMouseDownCapture={(e) => {
            if (e.button === 2 && hasSelection(session.id)) e.stopPropagation();
          }}
          onMouseUpCapture={(e) => {
            if (e.button === 2 && hasSelection(session.id)) e.stopPropagation();
          }}
          // Image paste uploads and writes a path in path mode, or explicitly sends Ctrl+V to the agent in native
          // mode. Leave non-image clipboard data to xterm's normal text-paste path.
          //
          // Capture is required: xterm's textarea paste listener unconditionally stops propagation, so a bubbling
          // onPaste never fires. This once broke image paste silently on desktop, browser, and remote clients.
          // When an image is found, also stop propagation to prevent xterm from performing an empty text paste.
          onPasteCapture={(e) => {
            const plan = planImagePaste(
              e.clipboardData,
              imagePasteMode,
              (env.isTauri || env.isElectron) && supportsNativeImagePaste(session.kind),
              env.isTauri,
            );
            if (plan.kind === "text") return;
            // Native mode cannot simply leave image paste to xterm: an image-only clipboard has no text/plain,
            // so Codex/Claude would never receive Ctrl+V. Supported TUIs take the agent branch; other sessions
            // fall back to path upload. Keep this guard against future routing changes sending control bytes.
            if (plan.kind === "agent" && !supportsNativeImagePaste(session.kind)) return;

            // Both image modes stop the captured event so xterm cannot perform an additional empty text paste.
            e.preventDefault();
            e.stopPropagation();
            if (plan.kind === "agent") {
              void injectNativeImagePaste(session.id).catch(() =>
                showImageError(t("term.imgClipboardUnavailable")),
              );
            } else if (plan.kind === "images") injectImages(plan.files);
            else if (plan.kind === "native-clipboard") injectNativeClipboardImage();
            else showImageError(t("term.imgClipboardUnavailable"));
          }}
          onDragOver={(e) => {
            if (e.dataTransfer?.types?.includes("Files")) e.preventDefault();
          }}
          onDrop={(e) => {
            const imgs = imagesFromDrop(e.dataTransfer);
            if (imgs.length) {
              e.preventDefault();
              injectImages(imgs);
            }
          }}
        >
          <div ref={containerRef} style={{ position: "absolute", top: 2, left: 5, right: 5, bottom: 2 }} />
          {/* Mirror-mode floating bar: this client is scaling the whole view to match a PTY sized by
              another client. Clicking takes over explicitly, resizing the PTY to this window and turning
              the other clients into mirrors. It is hidden in fit mode, the everyday single-client case. */}
          {sizeMode === "mirror" && !hidden && (
            <button
              title={t("term.mirrorTooltip")}
              onMouseDown={stop}
              onClick={(e) => {
                stop(e);
                takeoverSize();
              }}
              style={{
                position: "absolute",
                top: 8,
                right: 12,
                zIndex: 6,
                padding: "3px 10px",
                borderRadius: 6,
                border: "1px solid var(--text-faint)",
                background: "var(--bg-term)",
                color: "var(--text-dim)",
                fontSize: 12,
                opacity: 0.9,
                cursor: "pointer",
              }}
            >
              {t("term.mirrorBadge", ptyDims ? ` ${ptyDims.cols}×${ptyDims.rows}` : "")}
            </button>
          )}
          {/* Image upload failure banner, dismissed automatically after five seconds. */}
          {imgError && !hidden && (
            <div
              style={{
                position: "absolute",
                top: 8,
                left: "50%",
                transform: "translateX(-50%)",
                zIndex: 7,
                padding: "4px 12px",
                borderRadius: 6,
                background: "var(--red, #c0392b)",
                color: "#fff",
                fontSize: 12,
                pointerEvents: "none",
                maxWidth: "80%",
                whiteSpace: "nowrap",
                overflow: "hidden",
                textOverflow: "ellipsis",
              }}
            >
              {imgError}
            </div>
          )}
          {starting && !hidden && (
            <div
              style={{
                position: "absolute",
                inset: 0,
                zIndex: 5,
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                pointerEvents: "none",
                background: "var(--bg-term)",
                color: "var(--text-dim)",
                fontSize: 13,
                letterSpacing: 0.5,
              }}
            >
              {t(
                "term.starting",
                session.kind === "codex"
                  ? "Codex"
                  : session.kind === "opencode"
                    ? "OpenCode"
                    : session.kind === "copilot"
                      ? "Copilot"
                      : session.kind === "cursor"
                        ? "Cursor"
                        : session.kind === "antigravity"
                          ? "Antigravity"
                          : session.kind === "cline"
                            ? "Cline"
                            : session.kind === "pi"
                              ? "Pi"
                              : session.kind === "crush"
                                ? "Crush"
                                : session.kind === "kimi"
                                  ? "Kimi Code (K3)"
                                  : session.kind === "kiro"
                                    ? "Kiro"
                                  : session.kind === "grok"
                                    ? "Grok Build (Grok 4.5)"
                                  : session.kind === "zoo"
                                    ? "Zoo Code"
                                : "Claude",
              )}
            </div>
          )}
          {/* Guidance card for a missing agent, shown when runtime.agentMissing is set (agent sessions only). */}
          {!hidden && <AgentInstallCard session={session} />}
          <TermScrollbar containerRef={containerRef} hidden={hidden} />
        </div>
      </div>

      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={menuItems}
          onClose={() => setMenu(null)}
        />
      )}
    </div>
  );
});

/** Inline terminal shell selector: a header pill beside the split/close controls that displays the current shell
 *  and opens a switcher. It appears only for plain terminal sessions with a detected shell, not agent sessions.
 *  Switching uses store.switchSessionShell, shared with the context menu, and restarts a running session.
 *
 *  Keep it in the header rather than over xterm's lower-right corner. An overlay cannot be immediately clickable
 *  while also allowing text selection through the same mousedown; a pointer-through hover delay felt sluggish.
 *  The header placement provides immediate clicks and unobstructed selection everywhere in the terminal. */
function ShellPicker({ session }: { session: Session }) {
  const t = useT();
  const shells = useTermStore((s) => s.shells);
  const switchSessionShell = useTermStore((s) => s.switchSessionShell);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);

  if (session.kind !== "terminal" || shells.length === 0) return null;

  const current = shellDisplayName(shells, session.shell ?? null);
  const items: MenuItem[] = shells.map((sh) => ({
    label: (session.shell === sh.path ? "✓ " : "　") + sh.label,
    onClick: () => void switchSessionShell(session.id, sh.path),
  }));

  return (
    <>
      <button
        className="shell-pick"
        title={t("tree.shellMenu")}
        onMouseDown={(e) => e.stopPropagation()}
        onContextMenu={(e) => { e.preventDefault(); e.stopPropagation(); }}
        onClick={(e) => {
          e.stopPropagation();
          const r = e.currentTarget.getBoundingClientRect();
          // Header control: open below the pill; ContextMenu still clamps to viewport edges.
          setMenu({ x: r.left, y: r.bottom + 2 });
        }}
      >
        <Icons.terminal size={11} />
        <span className="shell-pick-name">{current}</span>
        <Icons.chevD size={9} />
      </button>
      {/* This must be portalled to body. The pill lives in the pane header, and `.pane` carries
          `contain: layout`, which makes it the containing block for position:fixed descendants and clips
          them with its own overflow:hidden. Rendered in place, the menu would be offset and cropped down
          to one or two clickable rows (in testing, only "cmd" could be selected). Portalled out of
          `.pane` it positions against viewport coordinates and is not clipped. */}
      {menu &&
        createPortal(
          <ContextMenu x={menu.x} y={menu.y} items={items} onClose={() => setMenu(null)} />,
          document.body,
        )}
    </>
  );
}

/** Fallback local-agent display names used before recipes arrive, preventing an empty title flash. */
const AGENT_KIND_LABEL: Partial<Record<Session["kind"], string>> = {
  claude: "Claude Code",
  codex: "Codex",
  opencode: "OpenCode",
  copilot: "GitHub Copilot CLI",
  cursor: "Cursor CLI",
  antigravity: "Antigravity CLI",
  cline: "Cline CLI",
  pi: "Pi",
  crush: "Crush",
  kimi: "Kimi Code (K3)",
  kiro: "Kiro",
  grok: "Grok Build (Grok 4.5)",
  zoo: "Zoo Code",
};

/** Locates an installation, fills the agent's global executable-path setting, and immediately flushes it to the
 *  backend. Returns whether a path already existed or was saved. Never overwrite a user-configured path. When
 *  `notify` is true, record agentPathSaved to show the installation-complete dialog; retry-start detection uses
 *  false because the user is already restarting and needs no additional dialog. */
async function locateAndSaveAgentPath(
  kind: Session["kind"],
  sessionId: string,
  notify: boolean,
): Promise<boolean> {
  const st = useTermStore.getState();
  const existing = st.agentDefaults[kind]?.path?.trim();
  if (existing) {
    if (notify) st.setRuntime(sessionId, { agentPathSaved: existing });
    return true;
  }
  const located = await agentLocateBin(kind);
  if (!located) return false;
  st.setAgentDefault(kind, { path: located });
  // Flush before showing the dialog or restarting; spawn reads app_settings immediately and cannot see a debounced value.
  await flushSettingsSync();
  if (notify) st.setRuntime(sessionId, { agentPathSaved: located });
  return true;
}

/** Installation-location polling for immediate post-install action. While runtime.agentInstalling is set, probe
 *  every three seconds and save the path as soon as the binary appears, then show the completion dialog.
 *
 *  Attach polling to TerminalView rather than AgentInstallCard, which unmounts when its pane is hidden. The
 *  keep-alive TerminalView continues across tab switches. Stop after success, after roughly 15 minutes (clearing
 *  agentInstalling and restoring the information card), or when the next spawn result resets the flag. Probing
 *  is lightweight—stat known directories and query npm prefix once—and an in-flight guard prevents overlap. */
function useAgentInstallLocator(session: Session) {
  const active = useTermStore((s) => {
    const rt = s.runtimes[session.id];
    return !!rt?.agentMissing && !!rt?.agentInstalling && !rt?.agentPathSaved;
  });
  const kind = session.kind;
  useEffect(() => {
    if (!active || !(kind in AGENT_KIND_LABEL)) return;
    let inFlight = false;
    let ticks = 0;
    const timer = setInterval(() => {
      if (inFlight) return;
      if (++ticks > 300) {
        useTermStore.getState().setRuntime(session.id, { agentInstalling: false });
        clearInterval(timer);
        return;
      }
      inFlight = true;
      locateAndSaveAgentPath(kind, session.id, true)
        .then((hit) => {
          if (hit) clearInterval(timer);
        })
        .catch(() => {})
        .finally(() => {
          inFlight = false;
        });
    }, 3000);
    return () => clearInterval(timer);
  }, [active, kind, session.id]);
}

/** Missing-agent guidance card shown for local agent sessions when runtime.agentMissing is set.
 *
 *  Three states: (1) an information panel overlays the idle terminal with a copyable recommended command,
 *  one-click install, retry, documentation, authentication guidance, and manual-install option; (2) one-click
 *  install writes the command to this session's shell, removes the card while output scrolls, and leaves location
 *  polling on TerminalView; (3) after the path is detected and saved, a completion dialog offers restart now or
 *  later. Restart increments the epoch, remounts the pane, and spawns using the new path. */
function AgentInstallCard({ session }: { session: Session }) {
  const t = useT();
  const missing = useTermStore((s) => !!s.runtimes[session.id]?.agentMissing);
  const restartSession = useTermStore((s) => s.restartSession);
  const setRuntime = useTermStore((s) => s.setRuntime);
  const [recipe, setRecipe] = useState<AgentInstallRecipe | null>(null);
  // Briefly show Copied after success so the action has visible feedback.
  const [copied, setCopied] = useState(false);
  // Keep installation state and the saved path in runtime state so they survive tab switches and pane remounts.
  const installing = useTermStore((s) => !!s.runtimes[session.id]?.agentInstalling);
  const pathSaved = useTermStore((s) => s.runtimes[session.id]?.agentPathSaved ?? null);

  const kind = session.kind;
  const eligible = kind in AGENT_KIND_LABEL;

  // Fetch OS-specific installation guidance once when a local agent is marked missing.
  useEffect(() => {
    if (!missing || !eligible) return;
    let alive = true;
    void agentInstallRecipe(kind)
      .then((r) => {
        if (alive) setRecipe(r);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [missing, eligible, kind]);

  if (!eligible || (!missing && !pathSaved)) return null;

  const label = recipe?.label ?? AGENT_KIND_LABEL[kind] ?? kind;
  const hasInstallCommand = !!recipe?.command.trim();
  const stop = (e: React.SyntheticEvent) => e.stopPropagation();
  // Dismissing the card clears installation polling. The hook reports not-found only once, so the card does not
  // reappear until a retry or restart runs detection again and confirms the agent is still missing.
  const dismiss = () => setRuntime(session.id, { agentMissing: false, agentInstalling: false });
  // Completion has two exits: restart now clears installation flags and starts with the new path; later dismisses
  // only the dialog because the saved setting will be used by any future start.
  const restartNow = () => {
    setRuntime(session.id, {
      agentMissing: false,
      agentInstalling: false,
      agentPathSaved: null,
    });
    void restartSession(session.id);
  };
  const later = () =>
    setRuntime(session.id, {
      agentMissing: false,
      agentInstalling: false,
      agentPathSaved: null,
    });
  // Clear flags before restarting to avoid flashing the old card during the epoch remount; detection restores it if needed.
  const retry = async () => {
    // Probe once more for users who installed manually and immediately clicked retry. Failure does not prevent
    // restart, which may still succeed after a new login shell reads the updated PATH.
    try {
      await locateAndSaveAgentPath(kind, session.id, false);
    } catch {
      /* Detection is best-effort; restart normally if it fails. */
    }
    setRuntime(session.id, { agentMissing: false, agentInstalling: false });
    void restartSession(session.id);
  };
  const doInstall = () => {
    if (!recipe) return;
    void ptyWrite(session.id, recipe.command + "\r");
    // Mark installation active, dismiss the card, and start TerminalView location polling.
    setRuntime(session.id, { agentInstalling: true });
    focusTerminal(session.id);
  };
  const openDocs = () => {
    if (recipe) void platform.opener.openExternal(recipe.docsUrl).catch(() => {});
  };
  const doCopy = () => {
    if (!recipe) return;
    void copyText(recipe.command);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1400);
  };

  const primaryBtn: React.CSSProperties = {
    padding: "5px 14px",
    borderRadius: 6,
    border: "1px solid var(--accent-line)",
    background: "var(--accent)",
    color: "var(--bg-0)",
    fontSize: 12,
    fontWeight: 600,
    cursor: "pointer",
  };
  const ghostBtn: React.CSSProperties = {
    padding: "5px 12px",
    borderRadius: 6,
    border: "1px solid var(--border-strong)",
    background: "transparent",
    color: "var(--text-secondary)",
    fontSize: 12,
    cursor: "pointer",
  };
  const linkBtn: React.CSSProperties = {
    padding: 0,
    border: "none",
    background: "transparent",
    color: "var(--text-dim)",
    fontSize: 11.5,
    cursor: "pointer",
    textDecoration: "underline",
  };

  // Installation complete: the path was detected and saved; prompt for restart.
  if (pathSaved) {
    return (
      <div
        onMouseDown={stop}
        style={{
          position: "absolute",
          inset: 0,
          zIndex: 8,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          padding: 16,
          overflow: "auto",
          background: "color-mix(in oklch, var(--bg-0) 82%, transparent)",
        }}
      >
        <div
          style={{
            width: "min(440px, 100%)",
            display: "flex",
            flexDirection: "column",
            gap: 12,
            padding: 18,
            borderRadius: 12,
            border: "1px solid var(--border)",
            background: "var(--bg-elevated)",
            boxShadow: "0 12px 40px rgba(0,0,0,0.4)",
          }}
        >
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span style={{ color: "var(--text-secondary)", display: "grid" }}>
              {kindIconEl(session.kind, 16)}
            </span>
            <span style={{ fontSize: 14, fontWeight: 600, color: "var(--text)" }}>
              {t("agentInstall.doneTitle", label)}
            </span>
          </div>
          <div style={{ fontSize: 12, lineHeight: 1.55, color: "var(--text-dim)" }}>
            {t("agentInstall.pathSaved", label)}{" "}
            <span
              style={{
                fontFamily: "var(--font-mono, monospace)",
                color: "var(--text-secondary)",
                overflowWrap: "anywhere",
              }}
            >
              {pathSaved}
            </span>
          </div>
          <div style={{ fontSize: 12, lineHeight: 1.55, color: "var(--text-dim)" }}>
            {t("agentInstall.doneDesc")}
          </div>
          <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
            <button style={ghostBtn} onMouseDown={stop} onClick={later}>
              {t("agentInstall.later")}
            </button>
            <button style={primaryBtn} onMouseDown={stop} onClick={restartNow}>
              {t("agentInstall.restartNow")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  // Installation in progress: show no overlay while output scrolls; TerminalView continues location polling.
  if (installing) return null;

  // Information state: centered guidance panel.
  return (
    <div
      onMouseDown={stop}
      style={{
        position: "absolute",
        inset: 0,
        zIndex: 8,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        padding: 16,
        overflow: "auto",
        background: "color-mix(in oklch, var(--bg-0) 82%, transparent)",
      }}
    >
      <div
        style={{
          width: "min(440px, 100%)",
          display: "flex",
          flexDirection: "column",
          gap: 12,
          padding: 18,
          borderRadius: 12,
          border: "1px solid var(--border)",
          background: "var(--bg-elevated)",
          boxShadow: "0 12px 40px rgba(0,0,0,0.4)",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span style={{ color: "var(--text-secondary)", display: "grid" }}>
            {kindIconEl(session.kind, 16)}
          </span>
          <span style={{ fontSize: 14, fontWeight: 600, color: "var(--text)" }}>
            {t("agentInstall.title", label)}
          </span>
          <span style={{ flex: 1 }} />
          {/* An explicit close button: while this covers the terminal the user needs an obvious way out, besides the "I'll install it myself" link in the corner. */}
          <button
            title={t("common.close")}
            aria-label={t("common.close")}
            style={{
              ...ghostBtn,
              padding: "2px 8px",
              fontSize: 14,
              lineHeight: 1,
              flex: "none",
            }}
            onMouseDown={stop}
            onClick={dismiss}
          >
            ✕
          </button>
        </div>

        <div style={{ fontSize: 12.5, color: "var(--text-dim)", lineHeight: 1.5 }}>
          {t("agentInstall.desc", label)}
        </div>

        {/* The recommended install command with a copy button. When the vendor does not support this platform the command is empty and only the limitation notice and documentation below are shown. */}
        {hasInstallCommand && (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              padding: "8px 10px",
              borderRadius: 8,
              border: "1px solid var(--border)",
              background: "var(--bg-0)",
            }}
          >
            <code
              style={{
                flex: 1,
                minWidth: 0,
                fontFamily: "var(--font-mono, ui-monospace, monospace)",
                fontSize: 12,
                color: "var(--text)",
                whiteSpace: "pre-wrap",
                wordBreak: "break-all",
              }}
            >
              {recipe?.command}
            </code>
            <button
              title={t("common.copy")}
              style={{
                ...ghostBtn,
                padding: "3px 8px",
                flex: "none",
                ...(copied ? { color: "var(--green)", borderColor: "var(--green)" } : null),
              }}
              onMouseDown={stop}
              onClick={doCopy}
            >
              {copied ? t("common.copied") : t("common.copy")}
            </button>
          </div>
        )}

        {recipe?.needsNode && (
          <div style={{ fontSize: 11.5, color: "var(--yellow)" }}>
            {t("agentInstall.needsNode")}
          </div>
        )}

        {/* Primary action. */}
        <div style={{ display: "flex", gap: 8 }}>
          {hasInstallCommand && (
            <button style={primaryBtn} onMouseDown={stop} onClick={doInstall}>
              {t("agentInstall.install")}
            </button>
          )}
          <button style={ghostBtn} onMouseDown={stop} onClick={retry}>
            {t("agentInstall.retry")}
          </button>
        </div>

        {/* Authentication note: installing the binary only solves half the problem. */}
        {recipe?.authHint && (
          <div style={{ fontSize: 11.5, color: "var(--text-dim)", lineHeight: 1.5 }}>
            <span style={{ color: "var(--text-secondary)" }}>
              {t("agentInstall.afterInstall")}
            </span>{" "}
            {recipe.authHint}
          </div>
        )}

        {/* Secondary links. */}
        <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
          <button style={linkBtn} onMouseDown={stop} onClick={openDocs} disabled={!recipe}>
            {t("agentInstall.docs")}
          </button>
          <span style={{ flex: 1 }} />
          <button style={linkBtn} onMouseDown={stop} onClick={dismiss}>
            {t("agentInstall.dismiss")}
          </button>
        </div>
      </div>
    </div>
  );
}

/** Custom terminal scrollbar that mirrors xterm viewport state as an always-visible floating control. Tauri's
 *  WKWebView on macOS does not render ::-webkit-scrollbar as a classic scrollbar, so a DOM replacement is needed. */
function TermScrollbar({
  containerRef,
  hidden,
}: {
  containerRef: React.RefObject<HTMLDivElement | null>;
  hidden: boolean;
}) {
  const [thumb, setThumb] = useState<{ top: number; height: number } | null>(null);
  const [hovered, setHovered] = useState(false);
  const dragging = useRef(false);
  const trackRef = useRef<HTMLDivElement | null>(null);

  const update = useCallback(() => {
    const el = containerRef.current;
    if (!el) return;
    const vp = el.querySelector<HTMLElement>(".xterm-viewport");
    if (!vp) return;
    const { scrollTop, scrollHeight, clientHeight } = vp;
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
  }, [containerRef]);

  useEffect(() => {
    if (hidden) {
      setThumb(null);
      return;
    }
    const el = containerRef.current;
    if (!el) return;

    let vp: HTMLElement | null = null;
    let scrollArea: HTMLElement | null = null;
    let raf = 0;

    const onScroll = () => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(update);
    };

    const bind = () => {
      vp = el.querySelector<HTMLElement>(".xterm-viewport");
      if (!vp) return;
      vp.addEventListener("scroll", onScroll, { passive: true });
      scrollArea = vp.querySelector<HTMLElement>(".xterm-scroll-area");
      if (scrollArea) mo.observe(scrollArea, { attributes: true, attributeFilter: ["style"] });
      update();
    };

    const mo = new MutationObserver(onScroll);
    const ro = new ResizeObserver(onScroll);
    ro.observe(el);

    const initRaf = requestAnimationFrame(bind);

    return () => {
      cancelAnimationFrame(initRaf);
      cancelAnimationFrame(raf);
      vp?.removeEventListener("scroll", onScroll);
      mo.disconnect();
      ro.disconnect();
    };
  }, [containerRef, hidden, update]);

  if (!thumb || hidden) return null;

  const scrollTo = (fraction: number) => {
    const vp = containerRef.current?.querySelector<HTMLElement>(".xterm-viewport");
    if (!vp) return;
    const clamped = Math.max(0, Math.min(1, fraction));
    vp.scrollTop = clamped * (vp.scrollHeight - vp.clientHeight);
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
        top: 3,
        right: 6,
        width: 8,
        bottom: 3,
        borderRadius: 4,
        background: hovered ? "var(--border-strong)" : "var(--border)",
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
