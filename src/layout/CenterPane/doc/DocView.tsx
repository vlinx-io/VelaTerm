import { safeError } from "../../../ipc/diagnosticSafety";
//! Document-tab body: loading/error state, header (name/path, mode switch, save), editor, external-change banner,
//! and close/conflict confirmation dialogs.
//!
//! To prevent formatting churn, component `text` is the **single source of truth** and the editor is only a view.
//! Pull text back from an editor only after an actual edit (`editedRef`); switching modes while merely reading
//! never invokes a serializer, preserving the source exactly. Reloads and mode changes rebuild via a new key.

import { useCallback, useEffect, useRef, useState } from "react";
import { getLocale, t as tt, useT } from "../../../i18n";
import { readTextFile, statFile, writeTextFile } from "../../../ipc/info";
import { isTauri } from "../../../ipc/transport";
import { env, platform } from "../../../platform";
import { DOC_EXPORT_PDF_EVENT, DOC_SAVE_EVENT } from "../../../hooks/useKeyboardShortcuts";
import { labelWithCombo } from "../../../hooks/shortcutRegistry";
import { findDocTab, useTermStore, type DocTab } from "../../../store/termStore";
import Icons from "../../../components/Icons";
import { ContextMenu, type MenuItem } from "../../../components/ContextMenu";
import { Splitter } from "../../../components/Splitter";
import { DocFileTree } from "./DocFileTree";
import { DocOutline, parseOutline, type OutlineHeading } from "./DocOutline";
import { DocSearchBar } from "./DocSearchBar";
import type { DocSearchControl } from "./docSearch";
import { ChangesView } from "../../../components/diff/ChangesView";
import { DiffDocView } from "./DiffDocView";
import { ImageDocView } from "./ImageDocView";
import { SourceEditor, type SourceHandle } from "./SourceEditor";
import { MarkdownEditor, type MarkdownHandle, type MarkdownViewMode } from "./MarkdownEditor";
import { ArtifactHeaderControls, ArtifactPane } from "./artifact/ArtifactParts";
import { useArtifact, type ArtifactState } from "./artifact/useArtifact";
import { countDocument, type DocStats } from "./docStats";
import { splitFrontMatter } from "./frontMatter";
import { toggleWritingMode, useWritingModes } from "./writingModes";
import "./docTheme.css";

/** External-change polling interval. Only the active tab stats mtime, a microsecond-scale operation. */
const POLL_MS = 2000;

/** Last sidebar width, inherited by new document tabs and independently adjustable thereafter. Visibility is
 *  not remembered; every tab starts hidden until explicitly opened. */
let lastSideWidth = 220;

/** Draggable sidebar width range. */
const SIDE_MIN_W = 150;
const SIDE_MAX_W = 480;

/** Debounce heading extraction while typing. */
const OUTLINE_DEBOUNCE_MS = 600;

/** Debounce status-bar statistics while typing; counting serializes the visual document. */
const STATS_DEBOUNCE_MS = 500;

/** Routes images to ImageViewer, HTML pages to the artifact preview, diffs to DiffDocView, comparisons to
 *  ChangesView, and markdown/code to TextDocView. */
export function DocView({ tab, hidden }: { tab: DocTab; hidden: boolean }) {
  if (tab.kind === "image") return <ImageDocView tab={tab} hidden={hidden} />;
  if (tab.kind === "diff") return <DiffDocView tab={tab} hidden={hidden} />;
  if (tab.kind === "changes") return <ChangesView tab={tab} hidden={hidden} />;
  if (tab.kind === "artifact") return <ArtifactDocView tab={tab} hidden={hidden} />;
  return <TextDocView tab={tab} hidden={hidden} />;
}

/** An artifact's mode, treating anything else a tab carried over (a Save As from a draft) as preview. */
export function artifactMode(mode: DocTab["mode"]): "preview" | "source" | "split" {
  return mode === "source" || mode === "split" ? mode : "preview";
}

/** HTML pages: the text editing path plus the sandboxed preview, whose state lives here so hooks stay unconditional. */
function ArtifactDocView({ tab, hidden }: { tab: DocTab; hidden: boolean }) {
  const artifact = useArtifact(tab.path, { hidden: hidden || artifactMode(tab.mode) === "source" });
  // Refresh from the tab menu, or `vopen` on an open page, reloads the preview as well as the text.
  const reload = artifact.reload;
  const firstNonce = useRef(tab.reloadNonce);
  useEffect(() => {
    if (tab.reloadNonce !== firstNonce.current) reload();
  }, [tab.reloadNonce, reload]);
  return <TextDocView tab={tab} hidden={hidden} artifact={artifact} />;
}

/** Split share of the source side, inherited by new artifact tabs. */
let lastSplitPct = 50;

const ARTIFACT_MODES = [
  { mode: "preview", label: "artifact.preview", Icon: Icons.eye },
  { mode: "source", label: "artifact.source", Icon: Icons.code },
  { mode: "split", label: "artifact.split", Icon: Icons.columns },
] as const;

function TextDocView({ tab, hidden, artifact }: { tab: DocTab; hidden: boolean; artifact?: ArtifactState }) {
  const t = useT();
  const setDocTabViewport = useTermStore((s) => s.setDocTabViewport);
  const artMode = artifact ? artifactMode(tab.mode) : null;
  const [consoleOpen, setConsoleOpen] = useState(false);
  const [splitPct, setSplitPct] = useState(lastSplitPct);
  const splitRef = useRef<HTMLDivElement | null>(null);
  const artifactRef = useRef(!!artifact);
  artifactRef.current = !!artifact;
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
  /** Status-bar statistics for Markdown documents. */
  const [stats, setStats] = useState<DocStats | null>(null);
  const statsTimerRef = useRef<number | null>(null);
  const writingModes = useWritingModes();
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
  const markdownRef = useRef<MarkdownHandle | null>(null);
  const useMarkdownEditor = tab.kind === "markdown" && !readonlyTrunc;
  const [editorReadyEpoch, setEditorReadyEpoch] = useState(0);
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
      useMarkdownEditor
        ? markdownRef.current?.getText()
        : sourceRef.current?.getText();
    return fromEditor ?? base;
  }, [useMarkdownEditor]);

  /** Gets search controls for the active editor mode, or null before it is ready. */
  const getSearchControl = useCallback(
    (): DocSearchControl | null =>
      (useMarkdownEditor ? markdownRef.current?.search : sourceRef.current?.search) ?? null,
    [useMarkdownEditor],
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
      if (statsTimerRef.current != null) clearTimeout(statsTimerRef.current);
    },
    [],
  );

  const refreshStats = useCallback(() => {
    const md = pullText();
    setStats(md == null ? null : countDocument(md));
  }, [pullText]);
  useEffect(() => {
    if (tab.kind === "markdown") refreshStats();
  }, [tab.kind, text, editorEpoch, refreshStats]);

  /** Outline navigation: source mode scrolls by line; preview scrolls to the Nth heading element. Both
   *  derive from the same markdown and skip fenced-code headings, so their indices align naturally. */
  const jumpToHeading = useCallback(
    (idx: number) => {
      const h = outline[idx];
      if (!h) return;
      if (useMarkdownEditor) markdownRef.current?.scrollToHeading(idx, h.line);
      else sourceRef.current?.scrollToLine(h.line);
    },
    [outline, useMarkdownEditor],
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
      rootRef.current?.querySelector(tab.mode === "source" ? ".doc-markdown-code .cm-scroller" : ".doc-markdown-visual") ??
      rootRef.current?.querySelector(".docview-body");
    const scrollTop = scroller?.scrollTop ?? 0;
    await load();
    requestAnimationFrame(() => {
      const el =
        rootRef.current?.querySelector(tab.mode === "source" ? ".doc-markdown-code .cm-scroller" : ".doc-markdown-visual") ??
        rootRef.current?.querySelector(".docview-body");
      if (el) el.scrollTop = scrollTop;
    });
  }, [load, tab.mode]);

  /** Artifacts follow the file without a banner: an agent rewriting the page is the expected case. */
  const quietReload = useCallback(async () => {
    try {
      const f = await readTextFile(tab.path);
      baselineMtimeRef.current = f.mtimeMs;
      editedRef.current = false;
      setTruncatedSize(f.truncated ? f.fullSize : null);
      setText(f.content);
      setExternalMtime(null);
      setEditorEpoch((n) => n + 1);
    } catch {
      /* The preview shows its own missing-file state; keep the last text here. */
    }
  }, [tab.path]);

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
    if (findDocTab(useTermStore.getState(), tab.id)?.dirty) {
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
        if (artifactRef.current && !findDocTab(useTermStore.getState(), tab.id)?.dirty) {
          void quietReload();
          return;
        }
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
  }, [hidden, loading, error, tab.path, tab.id, quietReload]);

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

  // Keep the editor mounted while changing its presentation mode.
  const switchMode = (mode: DocTab["mode"]) => {
    if (mode === tab.mode) return;
    // Mode switching rebuilds the editor, invalidating its search state; close the search bar until Cmd+F reopens it.
    setSearchOpen(false);
    // Record the reading position so the next view opens at the same place.
    if (tab.kind === "markdown") markdownRef.current?.prepareModeChange(mode as MarkdownViewMode);
    // Keep Markdown editor instances and their undo history across view changes.
    setDocTabMode(tab.id, mode);
  };

  /** Export current Markdown through the existing vector PDF renderer, independently of the active view. */
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
      // YAML front matter is metadata, not document content.
      pdfBlob = await buildDocPdfBlob(splitFrontMatter(content).body, theme);
    } catch (e) {
      console.error("[exportPdf] failed to generate the PDF:", safeError(e));
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
    const cur = findDocTab(useTermStore.getState(), tab.id);
    if (cur && !cur.dirty) setDocTabDirty(tab.id, true);
    // Refresh the visible outline after the edit debounce expires.
    if (outlineOnRef.current) {
      if (outlineTimerRef.current != null) clearTimeout(outlineTimerRef.current);
      outlineTimerRef.current = window.setTimeout(refreshOutline, OUTLINE_DEBOUNCE_MS);
    }
    if (tab.kind === "markdown") {
      if (statsTimerRef.current != null) clearTimeout(statsTimerRef.current);
      statsTimerRef.current = window.setTimeout(refreshStats, STATS_DEBOUNCE_MS);
    }
  }, [tab.id, tab.kind, setDocTabDirty, refreshOutline, refreshStats]);

  // Code files keep the existing clipboard menu. Markdown editors keep their native editing menus.
  const editableEl = () => rootRef.current?.querySelector<HTMLElement>(".cm-content") ?? null;
  const [editMenu, setEditMenu] = useState<{ x: number; y: number; hasSel: boolean } | null>(null);

  const onEditorContextMenu = (e: React.MouseEvent) => {
    // Handle only the document body; controls such as the search input retain their default behavior.
    if (useMarkdownEditor || !(e.target as HTMLElement).closest(".cm-content")) return;
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
      // execCommand targets the focused CodeMirror editor and its clipboard handlers.
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

  /** Loading, error, or the editor: the whole document area for text files, the source side for artifacts. */
  const textArea = (
    <>
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
          onContextMenu={onEditorContextMenu}
        >
          {/* Truncated files retain the existing read-only CodeMirror fallback. */}
          {useMarkdownEditor ? (
            <MarkdownEditor
              key={`md:${editorEpoch}`}
              ref={markdownRef}
              defaultValue={text}
              docPath={tab.path}
              mode={tab.mode as MarkdownViewMode}
              modes={writingModes}
              onEdited={onEdited}
              onReady={() => setEditorReadyEpoch(n => n + 1)}
              onImageError={showImagePasteError}
              onRequestSearch={() => setSearchOpen(true)}
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
    </>
  );

  const artifactPane = artifact && (
    <ArtifactPane
      state={artifact}
      path={tab.path}
      title={tab.title}
      viewport={tab.viewport ?? "fit"}
      consoleOpen={consoleOpen}
      onCloseConsole={() => setConsoleOpen(false)}
      onCloseTab={() => closeTab(tab.id)}
    />
  );

  return (
    <div
      ref={rootRef}
      className="docview"
      style={hidden ? { display: "none" } : undefined}
      onKeyDownCapture={(e) => {
        if (!useMarkdownEditor || e.nativeEvent.isComposing || e.altKey || e.shiftKey) return;
        if (!(e.metaKey || e.ctrlKey) || (e.key !== "/" && e.code !== "Slash")) return;
        if (!(e.target as HTMLElement).closest(".doc-markdown-pane")) return;
        // Handle this before CodeMirror consumes Mod-/ as a source-code comment command.
        e.preventDefault();
        e.stopPropagation();
        switchMode(tab.mode === "source" ? "visual" : "source");
      }}
      onKeyDown={(e) => {
        // F8 focus mode and F9 typewriter mode, Typora's defaults.
        if (useMarkdownEditor && !e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey && (e.key === "F8" || e.key === "F9")) {
          e.preventDefault();
          toggleWritingMode(e.key === "F8" ? "focus" : "typewriter");
          return;
        }
        // Cmd+F opens the shared search bar and stops propagation so global terminal search does not open.
        if ((e.metaKey || e.ctrlKey) && (e.key === "f" || e.key === "F" || e.code === "KeyF")) {
          e.preventDefault();
          e.stopPropagation();
          if (text != null && !error && artMode !== "preview") setSearchOpen(true);
        }
      }}
    >
      <div className="docview-head">
        {/* A draft has no directory yet, so the sidebar (outline and directory tree) would be meaningless; it appears once the file is written. */}
        {!isDraft && (
          <button
            className={"docview-treetoggle" + (sideVisible ? " on" : "")}
            title={t("doc.sidebar")}
            aria-label={t("doc.sidebar")}
            aria-expanded={sideVisible}
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
            {(["visual", "source", "compare"] as const).map(mode => (
              <button
                key={mode}
                className={(readonlyTrunc ? mode === "source" : tab.mode === mode) ? "on" : ""}
                disabled={readonlyTrunc}
                aria-pressed={readonlyTrunc ? mode === "source" : tab.mode === mode}
                onClick={() => switchMode(mode)}
              >
                {t(mode === "visual" ? "doc.visual" : mode === "compare" ? "doc.compare" : "doc.source")}
              </button>
            ))}
          </div>
        )}
        {artifact && (
          <ArtifactHeaderControls
            state={artifact}
            path={tab.path}
            viewport={artMode !== "source" ? tab.viewport ?? "fit" : undefined}
            onViewport={(v) => setDocTabViewport(tab.id, v)}
            consoleOpen={consoleOpen}
            onToggleConsole={() => setConsoleOpen((v) => !v)}
          />
        )}
        {artifact && (
          <div className="docview-seg">
            {ARTIFACT_MODES.map(({ mode, label, Icon }) => (
              <button key={mode} className={artMode === mode ? "on" : ""} aria-pressed={artMode === mode} onClick={() => switchMode(mode)}>
                <Icon size={12} />
                {t(label)}
              </button>
            ))}
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

        {artMode === "preview" ? (
          artifactPane
        ) : artMode === "split" ? (
          <div className="artifact-body" ref={splitRef}>
            <div className="artifact-source" style={{ width: `${splitPct}%` }}>
              {textArea}
            </div>
            <Splitter
              onDrag={(dx) => {
                const width = splitRef.current?.clientWidth ?? 0;
                if (width <= 0) return;
                setSplitPct((pct) => {
                  const next = Math.min(80, Math.max(20, pct + (dx / width) * 100));
                  lastSplitPct = next;
                  return next;
                });
              }}
            />
            {artifactPane}
          </div>
        ) : (
          textArea
        )}

        {/* The search bar is anchored to docview-main, which does not scroll, rather than inside
            docview-body, which does. Otherwise it would be part of the scrolled content, drift away with
            the text after navigating, and its buttons would become unreachable. */}
        {searchOpen && !loading && !error && text != null && (
          <DocSearchBar
            getControl={getSearchControl}
            epoch={editorEpoch + editorReadyEpoch}
            readOnly={readonlyTrunc}
            onClose={() => setSearchOpen(false)}
            onEdited={onEdited}
          />
        )}
      </div>

      {useMarkdownEditor && !loading && !error && text != null && (
        <div className="docview-status">
          <button
            className={writingModes.focus ? "on" : ""}
            aria-pressed={writingModes.focus}
            title={`${t("doc.focusMode")} (F8)`}
            onClick={() => toggleWritingMode("focus")}
          >
            {t("doc.focusMode")}
          </button>
          <button
            className={writingModes.typewriter ? "on" : ""}
            aria-pressed={writingModes.typewriter}
            title={`${t("doc.typewriterMode")} (F9)`}
            onClick={() => toggleWritingMode("typewriter")}
          >
            {t("doc.typewriterMode")}
          </button>
          {stats && <StatusStats stats={stats} />}
        </div>
      )}

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

/** Word, character, line, and reading-time counts for the status bar. */
function StatusStats({ stats }: { stats: DocStats }) {
  const t = useT();
  const format = new Intl.NumberFormat(getLocale()).format;
  const items = [
    t("doc.statWords", stats.words, format(stats.words)),
    t("doc.statCharacters", stats.characters, format(stats.characters)),
    t("doc.statLines", stats.lines, format(stats.lines)),
  ];
  if (stats.minutes > 0) items.push(t("doc.statMinutes", stats.minutes, format(stats.minutes)));
  return (
    <div className="docview-stats" role="group" aria-label={t("doc.statsLabel")}>
      {items.map((item) => <span key={item}>{item}</span>)}
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
