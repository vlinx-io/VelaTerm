//! Document-tab body: loading/error state, header (name/path, mode switch, save), editor, external-change banner,
//! and close/conflict confirmation dialogs.
//!
//! To prevent formatting churn, component `text` is the **single source of truth** and the editor is only a view.
//! Pull text back from an editor only after an actual edit (`editedRef`); switching modes while merely reading
//! never invokes a serializer, preserving the source exactly. Reloads and mode changes rebuild via a new key.

import { useCallback, useEffect, useRef, useState } from "react";
import { t as tt, useT } from "../../../i18n";
import { readTextFile, statFile, writeTextFile } from "../../../ipc/info";
import { isTauri } from "../../../ipc/transport";
import { env, platform } from "../../../platform";
import { DOC_EXPORT_PDF_EVENT, DOC_SAVE_EVENT } from "../../../hooks/useKeyboardShortcuts";
import { labelWithCombo } from "../../../hooks/shortcutRegistry";
import { useTermStore, type DocTab } from "../../../store/termStore";
import Icons from "../../../components/Icons";
import { ContextMenu, type MenuItem } from "../../../components/ContextMenu";
import { Splitter } from "../../../components/Splitter";
import { DocFileTree } from "./DocFileTree";
import { DocOutline, parseOutline, type OutlineHeading } from "./DocOutline";
import { DocSearchBar } from "./DocSearchBar";
import type { DocSearchControl } from "./docSearch";
import { ImageDocView } from "./ImageDocView";
import { SourceEditor, type SourceHandle } from "./SourceEditor";
import { WysiwygEditor, type WysiwygHandle } from "./WysiwygEditor";
import "./docTheme.css";

/** External-change polling interval. Only the active tab stats mtime, a microsecond-scale operation. */
const POLL_MS = 2000;

/** Last sidebar width, inherited by new document tabs and independently adjustable thereafter. Visibility is
 *  not remembered; every tab starts hidden until explicitly opened. */
let lastSideWidth = 220;

/** Draggable sidebar width range. */
const SIDE_MIN_W = 150;
const SIDE_MAX_W = 480;

/** Outline debounce after typing stops; WYSIWYG mode must serialize, so avoid doing it too often. */
const OUTLINE_DEBOUNCE_MS = 600;

/** Routes images to ImageViewer and markdown/code to the TextDocView editing path. */
export function DocView({ tab, hidden }: { tab: DocTab; hidden: boolean }) {
  if (tab.kind === "image") return <ImageDocView tab={tab} hidden={hidden} />;
  return <TextDocView tab={tab} hidden={hidden} />;
}

function TextDocView({ tab, hidden }: { tab: DocTab; hidden: boolean }) {
  const t = useT();
  const shortcutOverrides = useTermStore((s) => s.shortcutOverrides);
  const setDocTabMode = useTermStore((s) => s.setDocTabMode);
  const setDocTabDirty = useTermStore((s) => s.setDocTabDirty);
  const setDocTabPath = useTermStore((s) => s.setDocTabPath);
  const cancelCloseDocTab = useTermStore((s) => s.cancelCloseDocTab);
  const closeTab = useTermStore((s) => s.closeTab);

  // A new draft has no path: skip disk reads/polling and use Save As on first save.
  const isDraft = tab.path === "";

  // Content source of truth and I/O state live here; the store contains metadata only.
  const [text, setText] = useState<string | null>(null); // null until loading succeeds
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  /** Synchronous save guard; React state alone cannot block two handlers firing in the same key event. */
  const savingRef = useRef(false);
  /** Editor generation incremented on reload/mode switch and included in the editor key. */
  const [editorEpoch, setEditorEpoch] = useState(0);
  /** Disk mtime baseline for optimistic locking; successful local saves update it without raising a banner. */
  const baselineMtimeRef = useRef<number | null>(null);
  /** Whether the current editor instance was actually edited, controlling when text is pulled back. */
  const editedRef = useRef(false);
  /** Detected external change; non-null shows the banner and contains the new disk mtime. */
  const [externalMtime, setExternalMtime] = useState<number | null>(null);
  /** Disk mtime explicitly ignored by the user; suppress that version until the file changes again. */
  const ignoredMtimeRef = useRef<number | null>(null);
  /** Save-time mtime conflict that prompts for overwrite confirmation. */
  const [conflictPrompt, setConflictPrompt] = useState(false);
  /** Per-tab sidebar visibility initialized from the previous toggle state. */
  const [sideVisible, setSideVisible] = useState(false);
  /** Sidebar width adjusted by dragging and initialized from the previous width. */
  const [sideWidth, setSideWidth] = useState(lastSideWidth);
  /** Sidebar page: outline for markdown or directory tree; markdown defaults to outline. */
  const [sideTab, setSideTab] = useState<"outline" | "files">(
    tab.kind === "markdown" ? "outline" : "files",
  );
  /** Document heading outline, maintained only when the markdown outline page is visible. */
  const [outline, setOutline] = useState<OutlineHeading[]>([]);
  /** Shared search bar for both modes, opened with Cmd+F. */
  const [searchOpen, setSearchOpen] = useState(false);
  /** True file size when a file over 10 MB is truncated; non-null means read-only and unsavable. */
  const [truncatedSize, setTruncatedSize] = useState<number | null>(null);
  /** Source-editor image-paste failure message, auto-dismissing after five seconds like terminal paste. */
  const [imagePasteError, setImagePasteError] = useState<string | null>(null);
  const imagePasteErrorTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const readonlyTrunc = truncatedSize != null;

  const textRef = useRef<string | null>(null);
  textRef.current = text;
  /** Ref guard preventing save closures from overwriting a file with truncated content. */
  const truncatedRef = useRef(false);
  truncatedRef.current = readonlyTrunc;
  const wysiwygRef = useRef<WysiwygHandle | null>(null);
  const sourceRef = useRef<SourceHandle | null>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);

  const showImagePasteError = useCallback((message: string) => {
    setImagePasteError(message);
    if (imagePasteErrorTimerRef.current) clearTimeout(imagePasteErrorTimerRef.current);
    imagePasteErrorTimerRef.current = setTimeout(() => setImagePasteError(null), 5000);
  }, []);
  useEffect(
    () => () => {
      if (imagePasteErrorTimerRef.current) clearTimeout(imagePasteErrorTimerRef.current);
    },
    [],
  );

  /** Current content: pull through the serializer only after edits; otherwise return text unchanged. */
  const pullText = useCallback((): string | null => {
    const base = textRef.current;
    if (base == null) return null;
    if (!editedRef.current) return base;
    const fromEditor =
      tab.mode === "wysiwyg"
        ? wysiwygRef.current?.getMarkdown()
        : sourceRef.current?.getText();
    return fromEditor ?? base;
  }, [tab.mode]);

  /** Gets search controls for the active editor mode, or null before it is ready. */
  const getSearchControl = useCallback(
    (): DocSearchControl | null =>
      (tab.mode === "wysiwyg" ? wysiwygRef.current?.search : sourceRef.current?.search) ?? null,
    [tab.mode],
  );

  // Parse headings from current content; onEdited schedules debounced outline refreshes.
  const outlineOn = sideVisible && sideTab === "outline" && tab.kind === "markdown";
  const outlineOnRef = useRef(outlineOn);
  outlineOnRef.current = outlineOn;
  const outlineTimerRef = useRef<number | null>(null);
  const refreshOutline = useCallback(() => {
    const md = pullText();
    setOutline(md == null ? [] : parseOutline(md));
  }, [pullText]);

  // While the outline is open, reparse after source-of-truth changes or editor rebuilds.
  useEffect(() => {
    if (outlineOn) refreshOutline();
  }, [outlineOn, text, editorEpoch, refreshOutline]);
  useEffect(
    () => () => {
      if (outlineTimerRef.current != null) clearTimeout(outlineTimerRef.current);
    },
    [],
  );

  /** Outline navigation: source mode scrolls by line; WYSIWYG mode scrolls to the Nth heading element. Both
   *  derive from the same markdown and skip fenced-code headings, so their indices align naturally. */
  const jumpToHeading = useCallback(
    (idx: number) => {
      const h = outline[idx];
      if (!h) return;
      if (tab.mode === "source") {
        sourceRef.current?.scrollToLine(h.line);
        return;
      }
      const nodes = rootRef.current?.querySelectorAll<HTMLElement>(
        ".ProseMirror h1, .ProseMirror h2, .ProseMirror h3, .ProseMirror h4, .ProseMirror h5, .ProseMirror h6",
      );
      if (!nodes || nodes.length === 0) return;
      nodes[Math.min(idx, nodes.length - 1)].scrollIntoView({ block: "start" });
    },
    [outline, tab.mode],
  );

  // Loading and reloading.
  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const f = await readTextFile(tab.path);
      baselineMtimeRef.current = f.mtimeMs;
      ignoredMtimeRef.current = null;
      editedRef.current = false;
      setTruncatedSize(f.truncated ? f.fullSize : null);
      setText(f.content);
      setExternalMtime(null);
      setDocTabDirty(tab.id, false);
      setEditorEpoch((n) => n + 1);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [tab.path, tab.id, setDocTabDirty]);

  useEffect(() => {
    if (tab.path === "") {
      // Start a new draft empty without reading disk; first save supplies its real path through Save As.
      baselineMtimeRef.current = null;
      editedRef.current = false;
      setText("");
      setLoading(false);
    } else {
      void load();
    }
    // Load once on mount. Existing paths are immutable; a saved draft transitions in place through save logic.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /** Silently reloads an external change while preserving scroll position on a best-effort basis. */
  const reloadPreservingScroll = useCallback(async () => {
    const scroller =
      rootRef.current?.querySelector(".cm-scroller") ??
      rootRef.current?.querySelector(".docview-body");
    const scrollTop = scroller?.scrollTop ?? 0;
    await load();
    requestAnimationFrame(() => {
      const el =
        rootRef.current?.querySelector(".cm-scroller") ??
        rootRef.current?.querySelector(".docview-body");
      if (el) el.scrollTop = scrollTop;
    });
  }, [load]);

  // Explicit refresh (reloadNonce increments when viewing the same file again or choosing Refresh File).
  // With no edits, always reread disk because refresh explicitly asks for the latest content. With unsaved edits,
  // never discard silently; show the external-change banner only if disk truly changed. Clear any ignored-mtime
  // memory because an explicit refresh means the user wants to inspect the file now.
  const lastReloadNonceRef = useRef(tab.reloadNonce);
  useEffect(() => {
    if (tab.path === "") return; // Drafts have no disk file to reread.
    if (tab.reloadNonce === lastReloadNonceRef.current) return;
    lastReloadNonceRef.current = tab.reloadNonce;
    ignoredMtimeRef.current = null;
    if (useTermStore.getState().docTabs[tab.id]?.dirty) {
      void statFile(tab.path)
        .then((st) => {
          if (st.mtimeMs !== baselineMtimeRef.current) setExternalMtime(st.mtimeMs);
        })
        .catch(() => {
          /* Treat a deleted or temporarily unreadable file as unchanged; save reports its own error. */
        });
    } else {
      void reloadPreservingScroll();
    }
  }, [tab.reloadNonce, tab.id, tab.path, reloadPreservingScroll]);

  // External-change detection stats mtime every two seconds while active and error-free, pauses while hidden,
  // and checks immediately on return. **Notify but never auto-reload**: unexpected content replacement while
  // reading is disruptive. A changed mtime shows Reload/Ignore; an ignored version is not reported repeatedly.
  useEffect(() => {
    if (tab.path === "" || hidden || loading || error) return; // Drafts have no disk file to poll.
    let stopped = false;
    const check = async () => {
      try {
        const st = await statFile(tab.path);
        if (stopped) return;
        const baseline = baselineMtimeRef.current;
        if (baseline == null || st.mtimeMs === baseline) return;
        if (st.mtimeMs === ignoredMtimeRef.current) return;
        setExternalMtime(st.mtimeMs);
      } catch {
        /* Keep polling if the file is deleted or temporarily unreadable; save reports its own error. */
      }
    };
    void check(); // Check immediately when the tab becomes visible.
    const timer = setInterval(() => void check(), POLL_MS);
    return () => {
      stopped = true;
      clearInterval(timer);
    };
  }, [hidden, loading, error, tab.path]);

  // Save with optimistic mtime locking; force means the user confirmed overwriting an external change.
  const save = useCallback(
    async (opts?: { force?: boolean; thenClose?: boolean }): Promise<boolean> => {
      // Never save a truncated read-only view; it contains only the first 10 MB and would destroy the remainder.
      if (truncatedRef.current) return false;
      const content = pullText();
      if (content == null || savingRef.current) return false;

      // A new draft first obtains a destination, writes it, then becomes a normal document. Desktop uses native
      // Save As; browser and remote clients use the server-path picker because they lack a native dialog.
      if (tab.path === "") {
        savingRef.current = true;
        setSaving(true);
        try {
          const dest =
            isTauri || env.isElectron
              ? await platform.dialog.saveFile({
                  title: tt("doc.saveAsTitle"),
                  defaultPath: tab.title,
                })
              : await useTermStore.getState().promptSaveAs(tab.title);
          if (!dest) return false; // User canceled.
          const out = await writeTextFile(dest, content, null); // The file is new, so create it unconditionally.
          baselineMtimeRef.current = out.mtimeMs;
          editedRef.current = false; // The rebuilt editor has not been edited.
          setText(content);
          setExternalMtime(null);
          setDocTabPath(tab.id, dest); // Fill path/title/kind and clear isNew.
          setDocTabDirty(tab.id, false);
          setEditorEpoch((n) => n + 1); // Rebuild so SourceEditor selects highlighting for the new filename.
          if (opts?.thenClose) closeTab(tab.id);
          return true;
        } catch (e) {
          setError(tt("doc.saveFailed", String(e)));
          return false;
        } finally {
          savingRef.current = false;
          setSaving(false);
        }
      }

      savingRef.current = true;
      setSaving(true);
      try {
        const out = await writeTextFile(
          tab.path,
          content,
          opts?.force ? null : baselineMtimeRef.current,
        );
        if (out.conflict) {
          setConflictPrompt(true);
          return false;
        }
        baselineMtimeRef.current = out.mtimeMs;
        setText(content); // Update the source of truth without replacing the editor instance.
        setExternalMtime(null);
        setDocTabDirty(tab.id, false);
        if (opts?.thenClose) closeTab(tab.id);
        return true;
      } catch (e) {
        // Use module-level tt because save is a useCallback; avoid adding t to its dependency chain.
        setError(tt("doc.saveFailed", String(e)));
        return false;
      } finally {
        savingRef.current = false;
        setSaving(false);
      }
    },
    [pullText, tab.path, tab.id, tab.title, setDocTabDirty, setDocTabPath, closeTab],
  );

  // Cmd+S arrives from the global shortcut as a custom event carrying the tab ID.
  useEffect(() => {
    const onSave = (e: Event) => {
      if ((e as CustomEvent<string>).detail === tab.id) void save();
    };
    window.addEventListener(DOC_SAVE_EVENT, onSave);
    return () => window.removeEventListener(DOC_SAVE_EVENT, onSave);
  }, [tab.id, save]);

  // On mode switch, pull text only after edits to avoid serializer churn during reading, then rebuild the editor.
  const switchMode = (mode: DocTab["mode"]) => {
    if (mode === tab.mode) return;
    // Mode switching rebuilds the editor, invalidating its search state; close the search bar until Cmd+F reopens it.
    setSearchOpen(false);
    // The modes have different containers and heights, so preserve a scroll ratio to approximate reading position.
    const getScroller = () =>
      rootRef.current?.querySelector(".cm-scroller") ??
      rootRef.current?.querySelector(".docview-body");
    const from = getScroller();
    const ratio =
      from && from.scrollHeight > from.clientHeight
        ? from.scrollTop / (from.scrollHeight - from.clientHeight)
        : 0;
    const t = pullText();
    if (t != null) setText(t);
    editedRef.current = false; // The new editor instance has not been edited.
    setDocTabMode(tab.id, mode);
    setEditorEpoch((n) => n + 1);
    if (ratio > 0) {
      // WYSIWYG initializes asynchronously; poll for several frames until content becomes scrollable before restoring.
      let tries = 0;
      const restore = () => {
        const el = getScroller();
        if (el && el.scrollHeight > el.clientHeight) {
          el.scrollTop = ratio * (el.scrollHeight - el.clientHeight);
        } else if (++tries < 60) {
          requestAnimationFrame(restore);
        }
      };
      requestAnimationFrame(restore);
    }
  };

  /** Exports the current document as a vector PDF through react-pdf, with automatic pagination, consistent
   *  margins, unbroken text lines, and selectable text. WYSIWYG mode can provide ProseMirror HTML without
   *  serialization; source mode can render markdown in a temporary headless Crepe instance. Layout and pagination
   *  live in ./docPdf; this function supplies content, theme colors, and persistence. */
  const exportPdf = useCallback(async () => {
    const content = pullText();
    if (content == null || loading || !!error) return;

    // Pass raw markdown to docPdf, which tokenizes it with marked and maps tokens to react-pdf. Do not use the
    // live editor DOM because it contains CodeMirror toolbars, language selectors, line numbers, and drag handles.
    // pullText() returns current markdown source in both modes.

    // Resolve current theme colors for react-pdf, which understands hex/rgb but not oklch. Design tokens use
    // oklch(), so a 1 px canvas reliably converts arbitrary CSS colors to #rrggbb.
    const toHex = (cssColor: string, fallback: string): string => {
      try {
        const cv = document.createElement("canvas");
        cv.width = cv.height = 1;
        const x = cv.getContext("2d");
        if (!x) return fallback;
        x.fillStyle = fallback;
        x.fillStyle = cssColor; // Invalid values are ignored, preserving the fallback.
        x.fillRect(0, 0, 1, 1);
        const [r, g, b] = x.getImageData(0, 0, 1, 1).data;
        return "#" + [r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("");
      } catch {
        return fallback;
      }
    };
    const cssVar = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    // Match the editor: use --bg-term for the page and the corresponding text/link/code/border tokens.
    const theme = {
      bg: toHex(cssVar("--bg-term"), "#ffffff"),
      text: toHex(cssVar("--text"), "#1a1a1a"),
      muted: toHex(cssVar("--text-mid"), "#6b7280"),
      link: toHex(cssVar("--accent"), "#2563eb"),
      codeBg: toHex(cssVar("--bg-2"), "#f3f4f6"),
      codeText: toHex(cssVar("--mag") || cssVar("--text"), "#7c3aed"),
      border: toHex(cssVar("--border-strong") || cssVar("--border"), "#d1d5db"),
    };

    let pdfBlob: Blob | null = null;
    try {
      // Keep react-pdf and font payloads off the document-open path by loading them only during export.
      const { buildDocPdfBlob } = await import("./docPdf");
      pdfBlob = await buildDocPdfBlob(content, theme);
    } catch (e) {
      console.error("[exportPdf] failed to generate the PDF:", e);
      return;
    }

    if (!pdfBlob) return;

    const safeTitle = tab.title.replace(/[/\\:*?"<>|]/g, "_");

    // Browser/remote clients generate the PDF blob in local browser memory, so download it directly to that
    // client. Exported documents belong on the user's machine, not the server disk targeted by write_bytes_file.
    if (!isTauri && !env.isElectron) {
      const url = URL.createObjectURL(pdfBlob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${safeTitle}.pdf`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
      return;
    }

    // Desktop uses native Save As and write_bytes_file to persist on the local machine.
    const savePath = await platform.dialog.saveFile({
      defaultPath: `${safeTitle}.pdf`,
      filters: [{ name: tt("doc.pdfFilter"), extensions: ["pdf"] }],
    });
    if (!savePath) return;

    // Send Vec<u8> over backend IPC to the Rust write_bytes_file command.
    const bytes = [...new Uint8Array(await pdfBlob.arrayBuffer())];
    await platform.transport.invoke("write_bytes_file", { path: savePath, data: bytes });
  }, [pullText, tab.mode, tab.title, loading, error]);

  // PDF export arrives from the TabBar context menu as a custom event carrying the tab ID.
  useEffect(() => {
    const onExportPdf = (e: Event) => {
      if ((e as CustomEvent<string>).detail === tab.id) void exportPdf();
    };
    window.addEventListener(DOC_EXPORT_PDF_EVENT, onExportPdf);
    return () => window.removeEventListener(DOC_EXPORT_PDF_EVENT, onExportPdf);
  }, [tab.id, exportPdf]);

  const onEdited = useCallback(() => {
    editedRef.current = true;
    const cur = useTermStore.getState().docTabs[tab.id];
    if (cur && !cur.dirty) setDocTabDirty(tab.id, true);
    // Refresh the visible outline after the edit debounce expires.
    if (outlineOnRef.current) {
      if (outlineTimerRef.current != null) clearTimeout(outlineTimerRef.current);
      outlineTimerRef.current = window.setTimeout(refreshOutline, OUTLINE_DEBOUNCE_MS);
    }
  }, [tab.id, setDocTabDirty, refreshOutline]);

  /** Typora-like behavior: clicking whitespace beside or below the document focuses the editor and places the
   *  caret nearby. Handle only clicks on the container itself, leaving editor content and overlays unaffected. */
  const onBodyMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0) return; // Leave right-click to the custom menu without moving the caret or clearing selection.
    if (tab.mode !== "wysiwyg") return;
    const cls = (e.target as HTMLElement).classList;
    if (
      !cls.contains("docview-body") &&
      !cls.contains("docview-wysiwyg") &&
      !cls.contains("milkdown")
    )
      return;
    const pm = rootRef.current?.querySelector<HTMLElement>(".ProseMirror");
    if (!pm) return;
    e.preventDefault();
    // Clamp the click inside the document rectangle and use caretRangeFromPoint for the nearest text position.
    const rect = pm.getBoundingClientRect();
    const x = Math.min(Math.max(e.clientX, rect.left + 2), rect.right - 2);
    const y = Math.min(Math.max(e.clientY, rect.top + 2), rect.bottom - 2);
    const range = document.caretRangeFromPoint?.(x, y);
    pm.focus();
    const sel = window.getSelection();
    if (!sel) return;
    try {
      if (range) {
        sel.removeAllRanges();
        sel.addRange(range);
      } else {
        sel.selectAllChildren(pm);
        sel.collapseToEnd();
      }
    } catch {
      /* If caret placement fails, focus only and keep the interaction uninterrupted. */
    }
  };

  // Editor-body context menu replacing WebView's browser menu. Find the focused editable body: .ProseMirror
  // in WYSIWYG mode or CodeMirror's .cm-content in source mode.
  const editableEl = () =>
    rootRef.current?.querySelector<HTMLElement>(".ProseMirror, .cm-content") ?? null;
  const [editMenu, setEditMenu] = useState<{ x: number; y: number; hasSel: boolean } | null>(null);

  const onEditorContextMenu = (e: React.MouseEvent) => {
    // Handle only the document body; controls such as the search input retain their default behavior.
    if (!(e.target as HTMLElement).closest(".ProseMirror, .cm-content")) return;
    e.preventDefault();
    // The body still owns focus and its DOM selection; neither right-click nor ContextMenu mousedown steals it.
    setEditMenu({ x: e.clientX, y: e.clientY, hasSel: !!window.getSelection()?.toString() });
  };

  // Plain HTTP remote access forbids async clipboard reads, so disable menu paste and recommend Cmd+V.
  const canPaste = typeof navigator.clipboard?.readText === "function";
  const editMenuItems: MenuItem[] = [
    {
      label: t("common.cut"),
      disabled: !editMenu?.hasSel,
      // execCommand targets the focused editable element, allowing ProseMirror/CodeMirror handlers to serialize
      // cut/copy correctly, including rich structure from WYSIWYG mode.
      onClick: () => {
        document.execCommand("cut");
      },
    },
    {
      label: t("common.copy"),
      disabled: !editMenu?.hasSel,
      onClick: () => {
        document.execCommand("copy");
      },
    },
    {
      label: canPaste ? t("common.paste") : t("term.pasteUseShortcut"),
      disabled: !canPaste,
      onClick: () => {
        editableEl()?.focus();
        void platform.clipboard
          .readText()
          .then((txt) => {
            // insertText triggers beforeinput, which both editors use to insert or replace at the caret.
            if (txt) document.execCommand("insertText", false, txt);
          })
          .catch(() => {
            /* Silently ignore an unreadable clipboard. */
          });
      },
    },
    { label: "", separator: true },
    {
      label: t("common.selectAll"),
      onClick: () => {
        editableEl()?.focus();
        document.execCommand("selectAll");
      },
    },
  ];

  return (
    <div
      ref={rootRef}
      className="docview"
      style={hidden ? { display: "none" } : undefined}
      onKeyDown={(e) => {
        // Cmd+F opens the shared search bar and stops propagation so global terminal search does not open.
        if ((e.metaKey || e.ctrlKey) && (e.key === "f" || e.key === "F" || e.code === "KeyF")) {
          e.preventDefault();
          e.stopPropagation();
          if (text != null && !error) setSearchOpen(true);
        }
      }}
    >
      <div className="docview-head">
        {/* A draft has no directory yet, so the sidebar (outline and directory tree) would be meaningless; it appears once the file is written. */}
        {!isDraft && (
          <button
            className={"docview-treetoggle" + (sideVisible ? " on" : "")}
            title={t("doc.sidebar")}
            onClick={() => setSideVisible((v) => !v)}
          >
            {sideVisible ? <Icons.panelLeftFill size={14} /> : <Icons.panelLeft size={14} />}
          </button>
        )}
        <span className="title">{tab.title}</span>
        <span className="path" title={tab.path}>
          {isDraft ? t("doc.unsaved") : tab.path}
        </span>
        {tab.kind === "markdown" && (
          <div className="docview-seg">
            {/* Truncated read-only files are locked to the source view: switching to WYSIWYG would run incomplete markdown through the serializer. */}
            <button
              className={tab.mode === "wysiwyg" && !readonlyTrunc ? "on" : ""}
              disabled={readonlyTrunc}
              onClick={() => switchMode("wysiwyg")}
            >
              {t("doc.wysiwyg")}
            </button>
            <button
              className={tab.mode === "source" || readonlyTrunc ? "on" : ""}
              disabled={readonlyTrunc}
              onClick={() => switchMode("source")}
            >
              {t("doc.source")}
            </button>
          </div>
        )}
        <button
          className="docview-save"
          // Drafts can always be saved, including empty first saves; existing files save only when dirty.
          // Truncated views cannot save because writing incomplete content would remove the remainder.
          disabled={saving || loading || !!error || readonlyTrunc || (!tab.dirty && !isDraft)}
          title={labelWithCombo(t("doc.saveTooltip"), "saveDoc", shortcutOverrides)}
          onClick={() => void save()}
        >
          {saving ? t("doc.saving") : t("common.save")}
          {tab.dirty && !saving && (
            <span
              style={{
                width: 6,
                height: 6,
                borderRadius: "50%",
                background: "var(--accent)",
              }}
            />
          )}
        </button>
        {tab.kind === "markdown" && (
          <button
            className="docview-save"
            disabled={loading || !!error || readonlyTrunc}
            title={t("doc.exportPdf")}
            onClick={() => void exportPdf()}
          >
            <Icons.print size={13} />
            {t("doc.exportPdf")}
          </button>
        )}
      </div>

      {externalMtime != null && (
        <div className="docview-banner">
          {/* The wording depends on unsaved changes: dirty warns that reloading discards local edits, clean is an ordinary notice. */}
          <span style={{ flex: 1 }}>
            {tab.dirty ? t("doc.externalChanged") : t("doc.externalChangedClean")}
          </span>
          <button onClick={() => void reloadPreservingScroll()}>
            {tab.dirty ? t("doc.reloadDiscard") : t("doc.reload")}
          </button>
          <button
            onClick={() => {
              // Remember the ignored mtime and notify again only after another disk change.
              ignoredMtimeRef.current = externalMtime;
              setExternalMtime(null);
            }}
          >
            {t("doc.ignore")}
          </button>
        </div>
      )}

      {/* Truncated read-only notice: the file exceeds 10 MB, so only the first 10 MB was read, it opens read-only and saving is disabled. Informational only, with no actions. */}
      {readonlyTrunc && !loading && !error && (
        <div className="docview-banner">
          <span style={{ flex: 1 }}>
            {t("doc.truncatedReadonly", `${(truncatedSize / 1024 / 1024).toFixed(1)} MB`)}
          </span>
        </div>
      )}

      {imagePasteError && !loading && !error && (
        <div className="docview-banner">
          <span style={{ flex: 1 }}>{imagePasteError}</span>
          <button onClick={() => setImagePasteError(null)}>{t("common.close")}</button>
        </div>
      )}

      <div className="docview-main">
        {sideVisible && (
          <>
            <div className="docview-side" style={{ width: sideWidth }}>
              {tab.kind === "markdown" && (
                <div className="docview-side-tabs">
                  <button
                    className={sideTab === "outline" ? "on" : ""}
                    onClick={() => setSideTab("outline")}
                  >
                    {t("doc.outline")}
                  </button>
                  <button
                    className={sideTab === "files" ? "on" : ""}
                    onClick={() => setSideTab("files")}
                  >
                    {t("doc.fileTree")}
                  </button>
                </div>
              )}
              {outlineOn ? (
                <DocOutline headings={outline} onJump={jumpToHeading} />
              ) : (
                <DocFileTree docPath={tab.path} />
              )}
            </div>
            <Splitter
              onDrag={(dx) =>
                setSideWidth((w) => {
                  const next = Math.min(SIDE_MAX_W, Math.max(SIDE_MIN_W, w + dx));
                  lastSideWidth = next;
                  return next;
                })
              }
            />
          </>
        )}

        {loading && <div className="docview-state">{t("doc.loadingFile", tab.title)}</div>}

        {!loading && error && (
          <div className="docview-state">
            <div style={{ color: "var(--red)" }}>{error}</div>
            <div className="actions">
              <button className="vlx-btn" onClick={() => void load()}>
                {t("common.retry")}
              </button>
              <button className="vlx-btn" onClick={() => closeTab(tab.id)}>
                {t("doc.closeTab")}
              </button>
            </div>
          </div>
        )}

        {!loading && !error && text != null && (
          <div
            className="docview-body"
            onMouseDown={onBodyMouseDown}
            onContextMenu={onEditorContextMenu}
          >
            {/* Truncated read-only files are forced into the read-only source view, never WYSIWYG, to avoid serializing incomplete markdown. */}
            {tab.mode === "wysiwyg" && !readonlyTrunc ? (
              <WysiwygEditor
                key={`w:${editorEpoch}`}
                ref={wysiwygRef}
                defaultValue={text}
                docPath={tab.path}
                onEdited={onEdited}
              />
            ) : (
              <SourceEditor
                key={`s:${editorEpoch}`}
                ref={sourceRef}
                defaultValue={text}
                path={tab.path}
                kind={tab.kind === "markdown" ? "markdown" : "code"}
                onEdited={onEdited}
                onRequestSearch={() => setSearchOpen(true)}
                readOnly={readonlyTrunc}
                onImagePasteError={(failed, lastError) =>
                  showImagePasteError(t("term.imgUploadFailed", failed, lastError))
                }
                onImageClipboardUnavailable={() =>
                  showImagePasteError(t("term.imgClipboardUnavailable"))
                }
              />
            )}
          </div>
        )}

        {/* The search bar is anchored to docview-main, which does not scroll, rather than inside
            docview-body, which does. Otherwise it would be part of the scrolled content, drift away with
            the text after navigating, and its buttons would become unreachable. */}
        {searchOpen && !loading && !error && text != null && (
          <DocSearchBar
            getControl={getSearchControl}
            epoch={editorEpoch}
            onClose={() => setSearchOpen(false)}
            onEdited={onEdited}
          />
        )}
      </div>

      {editMenu && (
        <ContextMenu
          x={editMenu.x}
          y={editMenu.y}
          items={editMenuItems}
          onClose={() => setEditMenu(null)}
        />
      )}

      {tab.pendingClose && (
        <ConfirmModal
          title={t("doc.closeTitle")}
          body={t("doc.unsavedBody", tab.title)}
          actions={[
            {
              label: t("doc.saveAndClose"),
              primary: true,
              onClick: () => {
                cancelCloseDocTab(tab.id);
                void save({ thenClose: true });
              },
            },
            {
              label: t("doc.closeNoSave"),
              onClick: () => closeTab(tab.id),
            },
            { label: t("common.cancel"), onClick: () => cancelCloseDocTab(tab.id) },
          ]}
        />
      )}

      {conflictPrompt && (
        <ConfirmModal
          title={t("doc.conflictTitle")}
          body={t("doc.conflictBody")}
          actions={[
            {
              label: t("doc.overwrite"),
              primary: true,
              onClick: () => {
                setConflictPrompt(false);
                void save({ force: true });
              },
            },
            { label: t("common.cancel"), onClick: () => setConflictPrompt(false) },
          ]}
        />
      )}
    </div>
  );
}

/** Lightweight confirmation dialog matching LeftSidebar ConfirmDelete. Avoid window.confirm because native
 *  confirmation panels are unreliable under wry/WKWebView. */
function ConfirmModal({
  title,
  body,
  actions,
}: {
  title: string;
  body: string;
  actions: { label: string; primary?: boolean; onClick: () => void }[];
}) {
  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 1100,
        background: "rgba(0,0,0,0.45)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <div
        style={{
          width: 380,
          background: "var(--bg-panel)",
          border: "1px solid var(--border)",
          borderRadius: 10,
          padding: 18,
          boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div
          style={{
            fontSize: 14,
            fontWeight: 600,
            color: "var(--text-primary)",
            marginBottom: 10,
          }}
        >
          {title}
        </div>
        <div
          style={{
            fontSize: 13,
            color: "var(--text-secondary)",
            lineHeight: 1.6,
            marginBottom: 18,
          }}
        >
          {body}
        </div>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          {actions.map((a) => (
            <button
              key={a.label}
              className={"vlx-btn" + (a.primary ? " vlx-btn-primary" : "")}
              onClick={a.onClick}
            >
              {a.label}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
