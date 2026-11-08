//! Thin CodeMirror 6 source-mode wrapper. Markdown installs directly; code files match
//! @codemirror/language-data by filename and asynchronously inject language packages through a
//! Compartment, falling back to plain text. It forwards docChanged to onEdited and exposes getText.
//! DocView's unified search replaces CodeMirror's inconsistent built-in panel with custom highlights.
//! All theme values use Vlinx CSS variables and follow light/dark mode.

import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
} from "react";
import { basicSetup, EditorView } from "codemirror";
import { indentWithTab } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
import {
  LanguageDescription,
  syntaxHighlighting,
} from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import { Compartment, EditorState, Prec, RangeSetBuilder, StateEffect, StateField } from "@codemirror/state";
import { Decoration, type DecorationSet, keymap } from "@codemirror/view";
import { SearchCursor } from "@codemirror/search";
import { vlxHighlight } from "./docHighlight";
import { nativeSelectAll } from "./nativeSelectAll";
import { EMPTY_STATUS, type DocSearchControl, type SearchStatus } from "./docSearch";
import { uploadDocImage } from "../../../ipc/transport";
import { env } from "../../../platform";
import {
  extOf,
  imageFromNativeClipboard,
  imagesFromClipboard,
} from "../../../terminal/imageInput";

export interface SourceHandle {
  /** Restore keyboard focus without changing the current selection. */
  focus: () => void;
  /** Return the complete editor text, or null before initialization. */
  getText: () => string | null;
  /** Apply a companion editor update without reporting it as a user edit. */
  setText: (value: string) => void;
  /** Insert a note reference or attachment at the current selection. */
  insertText: (value: string) => void;
  /** Scroll to a zero-based line and place the cursor at its start, clamping to the final line. */
  scrollToLine: (line: number) => void;
  /** Unified find/replace controls shared with WYSIWYG mode. */
  search: DocSearchControl;
  /** Reading position: the caret line when on screen, otherwise the top line, as a fractional line. */
  readingPosition: () => SourcePosition | null;
  /** Scroll so the fractional line sits `offset` pixels below the top, optionally moving the caret. */
  revealPosition: (position: SourcePosition) => void;
  /** Toggle focus mode (dim everything but the current paragraph) and typewriter mode. */
  setWritingModes: (modes: { focus: boolean; typewriter: boolean }) => void;
}

export interface SourcePosition {
  /** Zero-based line; the fraction locates a point inside a soft-wrapped line. */
  line: number;
  offset: number;
  caret: boolean;
}

// ── Writing modes ──
const currentLineMark = Decoration.line({ class: "vlx-current-line" });
/** Lines of the paragraph holding the caret, bounded by blank lines. */
function currentParagraph(state: EditorState): DecorationSet {
  const doc = state.doc;
  const line = doc.lineAt(state.selection.main.head);
  let first = line.number;
  let last = line.number;
  if (line.text.trim()) {
    while (first > 1 && doc.line(first - 1).text.trim()) first--;
    while (last < doc.lines && doc.line(last + 1).text.trim()) last++;
  }
  const builder = new RangeSetBuilder<Decoration>();
  for (let n = first; n <= last; n++) builder.add(doc.line(n).from, doc.line(n).from, currentLineMark);
  return builder.finish();
}
const focusParagraph = StateField.define<DecorationSet>({
  create: currentParagraph,
  update: (deco, tr) => (tr.docChanged || tr.selection ? currentParagraph(tr.state) : deco),
  provide: (f) => EditorView.decorations.from(f),
});
/** Center the caret after typing and keyboard movement. Pointer selections and programmatic updates
 *  such as companion sync keep their scroll position. */
const typewriterScroll = EditorState.transactionExtender.of((tr) => {
  const typed = ["input", "delete", "undo", "redo", "move"].some((event) => tr.isUserEvent(event));
  const keyboardSelect = tr.isUserEvent("select") && !tr.isUserEvent("select.pointer");
  return typed || keyboardSelect
    ? { effects: EditorView.scrollIntoView(tr.newSelection.main.head, { y: "center" }) }
    : null;
});

// ── Custom search highlights held in a StateField and updated through setMatches ──
const vlxMatchMark = Decoration.mark({ class: "vlx-cm-match" });
const vlxCurrentMark = Decoration.mark({ class: "vlx-cm-match vlx-cm-match-current" });
const setMatches = StateEffect.define<{ ranges: { from: number; to: number }[]; current: number }>();
const matchField = StateField.define<DecorationSet>({
  create: () => Decoration.none,
  update(deco, tr) {
    deco = deco.map(tr.changes);
    for (const e of tr.effects) {
      if (e.is(setMatches)) {
        deco = Decoration.set(
          e.value.ranges.map((r, i) =>
            (i === e.value.current ? vlxCurrentMark : vlxMatchMark).range(r.from, r.to),
          ),
          true,
        );
      }
    }
    return deco;
  },
  provide: (f) => EditorView.decorations.from(f),
});

/** CodeMirror theme using CSS variables to follow Vlinx theme and accent changes. */
const cmTheme = EditorView.theme({
  "&": {
    height: "100%",
    fontSize: "13px",
    backgroundColor: "transparent",
    color: "var(--text)",
  },
  ".cm-content": {
    fontFamily: "var(--font-mono)",
    caretColor: "var(--accent)",
    padding: "16px 0",
  },
  ".cm-line": { padding: "0 12px" },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { overflow: "auto", lineHeight: "1.65" },
  ".cm-gutters": {
    backgroundColor: "transparent",
    color: "var(--text-faint)",
    border: "none",
  },
  ".cm-activeLineGutter": { backgroundColor: "var(--bg-hover)" },
  ".cm-activeLine": { backgroundColor: "var(--bg-hover)" },
  ".cm-cursor": { borderLeftColor: "var(--accent)" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": {
    backgroundColor: "var(--accent-soft) !important",
  },
  ".cm-selectionMatch": { backgroundColor: "var(--accent-soft)" },
  // Style the search panel with the Vlinx theme.
  ".cm-panels": {
    backgroundColor: "var(--bg-2)",
    color: "var(--text)",
    borderBottom: "1px solid var(--border)",
  },
  ".cm-panel.cm-search input": {
    background: "var(--bg-0)",
    border: "1px solid var(--border-strong)",
    borderRadius: "5px",
    color: "var(--text)",
    outline: "none",
  },
  ".cm-panel.cm-search button": {
    background: "var(--bg-1)",
    border: "1px solid var(--border)",
    borderRadius: "5px",
    color: "var(--text)",
  },
  ".cm-searchMatch": { backgroundColor: "var(--accent-soft)" },
  ".cm-searchMatch-selected": { backgroundColor: "var(--accent-line)" },
});

export const SourceEditor = forwardRef<
  SourceHandle,
  {
    defaultValue: string;
    /** Absolute path; code files match language by basename. */
    path: string;
    /** Markdown uses lang-markdown including fenced languages; code matches the filename registry. */
    kind: "markdown" | "code";
    /** Called after genuine user edits so DocView can mark dirty and decide persistence. */
    onEdited: () => void;
    /** Cmd+F request for DocView's unified search, replacing CodeMirror's panel. */
    onRequestSearch?: () => void;
    /** Read-only mode for truncated files over 10 MB; selection, copy, and search remain enabled. */
    readOnly?: boolean;
    /** Some or all pasted images failed; successful items are still inserted. */
    onImagePasteError?: (failed: number, lastError: string) => void;
    /** Native clipboard could not provide an image. */
    onImageClipboardUnavailable?: () => void;
  }
>(function SourceEditor({
  defaultValue,
  path,
  kind,
  onEdited,
  onRequestSearch,
  readOnly,
  onImagePasteError,
  onImageClipboardUnavailable,
}, ref) {
  const rootRef = useRef<HTMLDivElement | null>(null);
  const viewRef = useRef<EditorView | null>(null);
  const syncingRef = useRef(false);
  const onEditedRef = useRef(onEdited);
  onEditedRef.current = onEdited;
  const onRequestSearchRef = useRef(onRequestSearch);
  onRequestSearchRef.current = onRequestSearch;
  const onImagePasteErrorRef = useRef(onImagePasteError);
  onImagePasteErrorRef.current = onImagePasteError;
  const onImageClipboardUnavailableRef = useRef(onImageClipboardUnavailable);
  onImageClipboardUnavailableRef.current = onImageClipboardUnavailable;
  // Current query, case sensitivity, matches, and active index maintained in closure state.
  const searchRef = useRef<{
    query: string;
    caseSensitive: boolean;
    matches: { from: number; to: number }[];
    current: number;
  }>({ query: "", caseSensitive: false, matches: [], current: -1 });
  const focusCompartment = useRef(new Compartment()).current;
  const typewriterCompartment = useRef(new Compartment()).current;

  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    /** Unsaved drafts have no durable attachment directory, so embed data URLs to survive temp cleanup. */
    const fileDataUrl = (file: File) =>
      new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () =>
          typeof reader.result === "string"
            ? resolve(reader.result)
            : reject(new Error("Image data URL encoding failed"));
        reader.onerror = () => reject(reader.error ?? new Error("Image read failed"));
        reader.readAsDataURL(file);
      });

    /** Persist images individually and insert all successful items at the cursor; failures do not stop others. */
    const pasteImages = async (view: EditorView, files: File[]) => {
      const refs: string[] = [];
      let failed = 0;
      let lastError = "";
      for (const file of files) {
        try {
          const src = path
            ? await uploadDocImage(new Uint8Array(await file.arrayBuffer()), extOf(file), path)
            : await fileDataUrl(file);
          // Insert standard image syntax for Markdown and untyped drafts. Other text/code formats have
          // no universal image syntax, so insert the durable path rather than discarding pasted data.
          refs.push(kind === "markdown" || !path ? `![](${src})` : src);
        } catch (e) {
          failed += 1;
          lastError = e instanceof Error ? e.message : String(e);
        }
      }
      // The tab may switch or editor rebuild during upload; a stale instance must not modify the document.
      if (refs.length && viewRef.current === view) {
        view.dispatch(view.state.replaceSelection(refs.join("\n")));
        view.focus();
      }
      if (failed) onImagePasteErrorRef.current?.(failed, lastError);
    };

    // Mount code immediately as plain text and swap in an asynchronously loaded language through Compartment.
    const langCompartment = new Compartment();
    const view = new EditorView({
      parent: root,
      doc: defaultValue,
      extensions: [
        // Intercept Cmd+F before basicSetup's searchKeymap and delegate to unified search.
        Prec.highest(
          keymap.of([
            {
              key: "Mod-f",
              run: () => {
                onRequestSearchRef.current?.();
                return true;
              },
            },
          ]),
        ),
        basicSetup,
        // CodeMirror leaves Tab unbound for browser focus navigation. Source-editing convention instead
        // indents the line/selection with Tab and outdents with Shift+Tab.
        keymap.of([indentWithTab]),
        // The macOS menu's Select All and the context menu select only the rendered lines natively.
        nativeSelectAll(),
        matchField,
        focusCompartment.of([]),
        typewriterCompartment.of([]),
        langCompartment.of(
          kind === "markdown" ? markdown({ codeLanguages: languages }) : [],
        ),
        syntaxHighlighting(vlxHighlight),
        EditorView.lineWrapping,
        // Read-only disables state changes and editing while retaining selection/copy/search. It is fixed
        // at mount; truncated documents rebuild by key rather than switching at runtime.
        EditorState.readOnly.of(!!readOnly),
        EditorView.editable.of(!readOnly),
        EditorView.domEventHandlers({
          paste(event, view) {
            if (readOnly) return false;
            const files = imagesFromClipboard(event.clipboardData);
            if (files.length) {
              event.preventDefault();
              void pasteImages(view, files);
              return true;
            }

            // Leave text/HTML paste to CodeMirror. When WKWebView exposes neither a File nor text for a
            // pure image, read RGBA from the Tauri native clipboard and reuse the insertion path.
            const hasText = Array.from(event.clipboardData?.types ?? []).some((type) =>
              type.startsWith("text/"),
            );
            if (!hasText && env.isTauri) {
              event.preventDefault();
              void imageFromNativeClipboard()
                .then((file) => pasteImages(view, [file]))
                .catch(() => onImageClipboardUnavailableRef.current?.());
              return true;
            }
            return false;
          },
        }),
        EditorView.updateListener.of((u) => {
          if (u.docChanged && !syncingRef.current) onEditedRef.current();
        }),
        cmTheme,
      ],
    });
    viewRef.current = view;
    if (kind === "code") {
      const basename = path.split("/").pop() || path;
      const desc = LanguageDescription.matchFilename(languages, basename);
      if (desc) {
        desc
          .load()
          .then((support) => {
            // Discard a language package that resolves after unmount or replacement.
            if (viewRef.current !== view) return;
            view.dispatch({ effects: langCompartment.reconfigure(support) });
          })
          .catch(() => {
            /* Keep plain text if language loading fails without interrupting editing. */
          });
      }
    }
    return () => {
      viewRef.current = null;
      view.destroy();
    };
    // Create once per mount; the parent changes key for content or mode updates.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── Unified search: SearchCursor recomputation, custom highlights, navigation, and replacement ──
  const recompute = (view: EditorView) => {
    const st = searchRef.current;
    st.matches = [];
    if (!st.query) {
      st.current = -1;
      return;
    }
    const norm = st.caseSensitive ? undefined : (s: string) => s.toLowerCase();
    const cur = new SearchCursor(view.state.doc, st.query, 0, view.state.doc.length, norm);
    while (!cur.next().done) st.matches.push({ from: cur.value.from, to: cur.value.to });
  };
  /** Apply highlight decorations and optionally select/center the active match. */
  const renderMatches = (view: EditorView, scroll: boolean) => {
    const st = searchRef.current;
    const effects: StateEffect<unknown>[] = [
      setMatches.of({ ranges: st.matches, current: st.current }),
    ];
    const spec: Parameters<EditorView["dispatch"]>[0] = { effects };
    if (scroll && st.current >= 0) {
      const m = st.matches[st.current];
      spec.selection = { anchor: m.from, head: m.to };
      effects.push(EditorView.scrollIntoView(m.from, { y: "center" }));
    }
    view.dispatch(spec);
  };
  const searchStatus = (): SearchStatus => {
    const st = searchRef.current;
    return st.matches.length ? { total: st.matches.length, current: st.current + 1 } : EMPTY_STATUS;
  };
  /** Select the first match after pos, wrapping to the first when necessary. */
  const locateFrom = (pos: number) => {
    const st = searchRef.current;
    st.current = st.matches.findIndex((m) => m.from >= pos);
    if (st.current === -1 && st.matches.length) st.current = 0;
  };
  const moveBy = (dir: 1 | -1): SearchStatus => {
    const view = viewRef.current;
    const st = searchRef.current;
    if (!view || !st.matches.length) return EMPTY_STATUS;
    const n = st.matches.length;
    st.current = (st.current + dir + n) % n;
    renderMatches(view, true);
    return searchStatus();
  };

  useImperativeHandle(ref, () => ({
    focus: () => viewRef.current?.focus(),
    getText: () => viewRef.current?.state.doc.toString() ?? null,
    insertText: (value) => {
      const view = viewRef.current;
      if (!view) return;
      view.dispatch(view.state.replaceSelection(value));
      view.focus();
    },
    setText: (value) => {
      const view = viewRef.current;
      if (!view || view.state.doc.toString() === value) return;
      syncingRef.current = true;
      try {
        view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: value } });
      } finally { syncingRef.current = false; }
    },
    scrollToLine: (line) => {
      const view = viewRef.current;
      if (!view) return;
      const n = Math.max(1, Math.min(line + 1, view.state.doc.lines));
      const pos = view.state.doc.line(n).from;
      view.dispatch({
        selection: { anchor: pos },
        effects: EditorView.scrollIntoView(pos, { y: "start", yMargin: 12 }),
      });
    },
    readingPosition: () => {
      const view = viewRef.current;
      if (!view) return null;
      const box = view.scrollDOM.getBoundingClientRect();
      const head = view.state.selection.main.head;
      const coords = view.coordsAtPos(head);
      const caret = !!coords && coords.top >= box.top && coords.bottom <= box.bottom;
      const y = caret ? coords.top : box.top + 1;
      const block = caret ? view.lineBlockAt(head) : view.lineBlockAtHeight(y - view.documentTop);
      const line = view.state.doc.lineAt(block.from);
      const within = block.height > 0 ? Math.min(0.999, Math.max(0, (y - view.documentTop - block.top) / block.height)) : 0;
      return { line: line.number - 1 + within, offset: y - box.top, caret };
    },
    revealPosition: ({ line, offset, caret }) => {
      const view = viewRef.current;
      if (!view) return;
      const doc = view.state.doc;
      const target = doc.line(Math.max(1, Math.min(doc.lines, Math.floor(line) + 1)));
      const pos = target.from + Math.round((line - Math.floor(line)) * target.length);
      view.dispatch({
        selection: caret ? { anchor: pos } : undefined,
        effects: EditorView.scrollIntoView(pos, { y: "start", yMargin: Math.max(0, offset) }),
      });
    },
    setWritingModes: ({ focus, typewriter }) => {
      viewRef.current?.dispatch({
        effects: [
          focusCompartment.reconfigure(focus ? focusParagraph : []),
          typewriterCompartment.reconfigure(typewriter ? typewriterScroll : []),
        ],
      });
    },
    search: {
      apply: ({ query, caseSensitive }) => {
        const view = viewRef.current;
        if (!view) return EMPTY_STATUS;
        const st = searchRef.current;
        st.query = query;
        st.caseSensitive = caseSensitive;
        recompute(view);
        locateFrom(view.state.selection.main.head);
        renderMatches(view, true);
        return searchStatus();
      },
      next: () => moveBy(1),
      prev: () => moveBy(-1),
      replace: (replaceText) => {
        const view = viewRef.current;
        const st = searchRef.current;
        if (!view || !st.matches.length || st.current < 0) return searchStatus();
        const m = st.matches[st.current];
        view.dispatch({ changes: { from: m.from, to: m.to, insert: replaceText } });
        recompute(view);
        locateFrom(m.from + replaceText.length);
        renderMatches(view, true);
        return searchStatus();
      },
      replaceAll: (replaceText) => {
        const view = viewRef.current;
        const st = searchRef.current;
        if (!view || !st.matches.length) return searchStatus();
        view.dispatch({
          changes: st.matches.map((m) => ({ from: m.from, to: m.to, insert: replaceText })),
        });
        recompute(view);
        st.current = st.matches.length ? 0 : -1;
        renderMatches(view, true);
        return searchStatus();
      },
      clear: () => {
        const view = viewRef.current;
        const st = searchRef.current;
        st.query = "";
        st.matches = [];
        st.current = -1;
        if (view) view.dispatch({ effects: setMatches.of({ ranges: [], current: -1 }) });
      },
    },
  }));

  return <div className="docview-source" ref={rootRef} />;
});
