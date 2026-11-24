//! Read-only, side-by-side diff of one file in a CodeMirror MergeView, shared by the Changes tab
//! and the single-file diff document tab. Compares the worktree, one commit, or a commit range,
//! depending on props, and optionally hosts inline review comments anchored to exact diff lines.

import { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { MergeView } from "@codemirror/merge";
import type { Text } from "@codemirror/state";
import { useT } from "../../i18n";
import { attachChatInputGuard } from "../../layout/CenterPane/session/inputGuard";
import { diffIgnoringWhitespace } from "./whitespaceDiff";
import {
  extractLineSnippet,
  formatLineRange,
  useDiffReviewStore,
  useScopeComments,
  type DiffCommentSide,
  type DiffReviewComment,
} from "./diffReviewStore";
import {
  gitCommitFileDiff,
  gitRangeFileDiff,
  gitFileDiff,
  type FileDiff,
  type GitChangeSide,
} from "../../ipc/commands";

/** Viewer preferences, kept across openings. */
export interface DiffOptions {
  /** Treat lines that differ only in whitespace as unchanged. */
  ignoreWhitespace: boolean;
  /** Fold long runs of unchanged lines. */
  collapseUnchanged: boolean;
}

export interface DiffSelectionRange {
  side: DiffCommentSide;
  startLine: number;
  endLine: number;
}

export interface DiffDraftRequest {
  nonce: number;
  side: DiffCommentSide;
  startLine: number;
  endLine: number;
}

interface ActiveDraft {
  side: DiffCommentSide;
  startLine: number;
  endLine: number;
  editingId?: string;
  initialBody: string;
}

const OPTIONS_KEY = "vlx.diff.options";

export function readDiffOptions(): DiffOptions {
  try {
    const saved = JSON.parse(localStorage.getItem(OPTIONS_KEY) ?? "{}") as Partial<DiffOptions>;
    return { ignoreWhitespace: saved.ignoreWhitespace === true, collapseUnchanged: saved.collapseUnchanged !== false };
  } catch {
    return { ignoreWhitespace: false, collapseUnchanged: true };
  }
}

export function writeDiffOptions(options: DiffOptions): void {
  try {
    localStorage.setItem(OPTIONS_KEY, JSON.stringify(options));
  } catch {
    // The choice then lasts while the dialog is open.
  }
}

function InlineCommentForm({
  draft,
  onSave,
  onCancel,
}: {
  draft: ActiveDraft;
  onSave: (body: string) => void;
  onCancel: () => void;
}) {
  const t = useT();
  const inputId = useId();
  const rootRef = useRef<HTMLFormElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const lastCompositionEndAtRef = useRef<number | null>(null);
  const [text, setText] = useState(draft.initialBody);

  useEffect(() => {
    setText(draft.initialBody);
  }, [draft.initialBody, draft.side, draft.startLine, draft.endLine, draft.editingId]);

  useEffect(() => attachChatInputGuard(rootRef.current as unknown as HTMLDivElement | null), []);

  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.focus();
    el.setSelectionRange(el.value.length, el.value.length);
  }, [draft.side, draft.startLine, draft.endLine, draft.editingId]);

  const submit = () => {
    const trimmed = text.trim();
    if (!trimmed) return;
    onSave(trimmed);
  };

  const sideLabel =
    draft.side === "original" ? t("changes.review.sideOriginal") : t("changes.review.sideModified");

  return (
    <form
      ref={rootRef}
      className="diff-comment-card diff-comment-form"
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <div className="diff-comment-card-head">
        <label htmlFor={inputId} className="diff-comment-label">
          {t("changes.review.fieldLabel")} · {sideLabel} {formatLineRange(draft.startLine, draft.endLine)}
        </label>
      </div>
      <textarea
        ref={textareaRef}
        id={inputId}
        name="comment"
        className="vlx-input diff-comment-textarea"
        rows={3}
        value={text}
        placeholder={t("changes.review.placeholder")}
        onChange={(e) => setText(e.target.value)}
        onCompositionEnd={(e) => {
          lastCompositionEndAtRef.current = e.nativeEvent.timeStamp;
        }}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            onCancel();
            return;
          }
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            const native = e.nativeEvent;
            if (native.isComposing || native.keyCode === 229) return;
            if (
              lastCompositionEndAtRef.current !== null &&
              Math.abs(native.timeStamp - lastCompositionEndAtRef.current) < 50
            ) {
              return;
            }
            submit();
          }
        }}
      />
      <div className="diff-comment-actions">
        <button type="button" className="vlx-btn" onClick={onCancel}>
          {t("changes.review.cancel")}
        </button>
        <button type="submit" className="vlx-btn vlx-btn-primary" disabled={!text.trim()}>
          {t("changes.review.save")}
        </button>
      </div>
    </form>
  );
}

function SavedCommentCard({
  comment,
  onEdit,
  onRemove,
}: {
  comment: DiffReviewComment;
  onEdit: () => void;
  onRemove: () => void;
}) {
  const t = useT();
  const sideLabel =
    comment.side === "original" ? t("changes.review.sideOriginal") : t("changes.review.sideModified");
  const range = formatLineRange(comment.startLine, comment.endLine);

  return (
    <div className="diff-comment-card">
      <div className="diff-comment-card-head">
        <span className="diff-comment-meta">
          {sideLabel} · {range}
        </span>
        <div className="diff-comment-head-btns">
          <button
            type="button"
            className="vlx-btn"
            aria-label={`${t("changes.review.edit")} ${range}`}
            onClick={onEdit}
          >
            {t("changes.review.edit")}
          </button>
          <button
            type="button"
            className="vlx-btn"
            aria-label={`${t("changes.review.remove")} ${range}`}
            onClick={onRemove}
          >
            {t("changes.review.remove")}
          </button>
        </div>
      </div>
      <div className="diff-comment-body">{comment.body}</div>
    </div>
  );
}

/**
 * Render a single-file diff by loading both texts into a read-only MergeView, adding language support
 * transparently once loaded. `tick` is the modal's refresh counter: it carries no data, it only forces
 * the open diff to reload so it cannot disagree with the refreshed line counts beside it.
 */
export function FileDiffView({
  cwd,
  path,
  tick,
  commit,
  base,
  side,
  options,
  reviewScope,
  draftRequest,
  onSelectionChange,
}: {
  cwd: string;
  path: string;
  tick: number;
  options: DiffOptions;
  commit: string | null;
  /** Left side of a range comparison; the commit's parent when null. */
  base: string | null;
  side: GitChangeSide;
  /** Enables inline line-review comments stored under this comparison key. */
  reviewScope?: string;
  /** External request from the file header button to open a comment form on the given line span. */
  draftRequest?: DiffDraftRequest | null;
  /** Reports non-empty line selections inside either diff pane so the header button can offer a range label. */
  onSelectionChange?: (selection: DiffSelectionRange | null) => void;
}) {
  const t = useT();
  const unchangedLines = t("changes.unchangedLines");
  const ref = useRef<HTMLDivElement>(null);
  const [diff, setDiff] = useState<FileDiff | null>(null);
  const [err, setErr] = useState("");
  const [draft, setDraft] = useState<ActiveDraft | null>(null);
  const [widgetHosts, setWidgetHosts] = useState<Record<string, HTMLElement>>({});

  const scopeComments = useScopeComments(reviewScope);
  const fileComments = useMemo(
    () => scopeComments.filter((c) => c.path === path),
    [scopeComments, path],
  );
  const addComment = useDiffReviewStore((s) => s.addComment);
  const updateComment = useDiffReviewStore((s) => s.updateComment);
  const removeComment = useDiffReviewStore((s) => s.removeComment);

  const syncDecorationsRef = useRef<(() => void) | null>(null);
  const draftRef = useRef<ActiveDraft | null>(null);
  draftRef.current = draft;
  const fileCommentsRef = useRef<DiffReviewComment[]>(fileComments);
  fileCommentsRef.current = fileComments;
  const tRef = useRef(t);
  tRef.current = t;
  const onSelectionRef = useRef(onSelectionChange);
  onSelectionRef.current = onSelectionChange;

  const registerHost = useCallback((key: string, el: HTMLElement | null) => {
    setWidgetHosts((prev) => {
      if (el) {
        return prev[key] === el ? prev : { ...prev, [key]: el };
      }
      if (!(key in prev)) return prev;
      const next = { ...prev };
      delete next[key];
      return next;
    });
  }, []);

  useEffect(() => {
    setDraft(null);
    onSelectionRef.current?.(null);
  }, [cwd, path, commit, base, side]);

  useEffect(() => {
    if (!draftRequest || !reviewScope) return;
    setDraft({
      side: draftRequest.side,
      startLine: draftRequest.startLine,
      endLine: draftRequest.endLine,
      initialBody: "",
    });
  }, [draftRequest, reviewScope]);

  useEffect(() => {
    let alive = true;
    setDiff(null);
    setErr("");
    const load = commit && base ? gitRangeFileDiff(cwd, base, commit, path)
      : commit ? gitCommitFileDiff(cwd, commit, path) : gitFileDiff(cwd, path, side);
    void load
      .then((d) => {
        if (alive) setDiff(d);
      })
      .catch((e) => {
        if (alive) setErr(String(e));
      });
    return () => {
      alive = false;
    };
  }, [cwd, path, tick, commit, base, side]);

  useEffect(() => {
    syncDecorationsRef.current?.();
  }, [fileComments, draft]);

  useEffect(() => {
    const el = ref.current;
    if (!el || !diff || diff.binary || diff.tooLarge) return;
    let mv: MergeView | null = null;
    let cancelled = false;
    setWidgetHosts({});
    void (async () => {
      // CodeMirror and the language catalogue load on demand: the modal is opened rarely, and a static
      // import would place roughly 330 kB of editor code in the entry chunk.
      const [merge, view, state, theme] = await Promise.all([
        import("@codemirror/merge"),
        import("@codemirror/view"),
        import("@codemirror/state"),
        import("./codeMirrorTheme"),
      ]);
      const { MergeView, diff: charDiff, Change } = merge;
      const { Decoration, EditorView, GutterMarker, WidgetType, gutter, lineNumbers } = view;
      const { EditorState, RangeSetBuilder, StateEffect, StateField } = state;
      const langExt = await theme.languageExtensionFor(diff.path);
      if (cancelled || !ref.current) return;

      const rebuildEffect = StateEffect.define<null>();

      class PortalBlockWidget extends WidgetType {
        constructor(readonly hostKey: string) {
          super();
        }
        eq(other: PortalBlockWidget) {
          return this.hostKey === other.hostKey;
        }
        toDOM() {
          const wrap = document.createElement("div");
          wrap.className = "cm-diff-widget-host";
          registerHost(this.hostKey, wrap);
          return wrap;
        }
        destroy() {
          registerHost(this.hostKey, null);
        }
        ignoreEvent() {
          return true;
        }
      }

      class AddCommentMarker extends GutterMarker {
        constructor(readonly lineNo: number) {
          super();
        }
        eq(other: AddCommentMarker) {
          return this.lineNo === other.lineNo;
        }
        toDOM() {
          const span = document.createElement("span");
          span.className = "cm-diff-comment-plus";
          span.textContent = "+";
          span.title = tRef.current("changes.review.gutterHint", this.lineNo);
          span.setAttribute("aria-hidden", "true");
          return span;
        }
      }

      const buildDecorationsForSide = (doc: Text, paneSide: DiffCommentSide) => {
        const entries: { from: number; to: number; value: ReturnType<typeof Decoration.line> }[] = [];
        if (reviewScope && doc.lines > 0) {
          for (const c of fileCommentsRef.current) {
            if (c.side !== paneSide) continue;
            const fromLine = Math.max(1, Math.min(c.startLine, doc.lines));
            const toLine = Math.max(fromLine, Math.min(c.endLine, doc.lines));
            for (let ln = fromLine; ln <= toLine; ln++) {
              const lineObj = doc.line(ln);
              entries.push({
                from: lineObj.from,
                to: lineObj.from,
                value: Decoration.line({ class: "cm-diff-commented-line" }),
              });
            }
            const anchor = doc.line(toLine);
            entries.push({
              from: anchor.to,
              to: anchor.to,
              value: Decoration.widget({
                widget: new PortalBlockWidget(`comment:${c.id}`),
                block: true,
                side: 1,
              }),
            });
          }

          const currentDraft = draftRef.current;
          if (currentDraft && currentDraft.side === paneSide) {
            const fromLine = Math.max(1, Math.min(currentDraft.startLine, doc.lines));
            const toLine = Math.max(fromLine, Math.min(currentDraft.endLine, doc.lines));
            for (let ln = fromLine; ln <= toLine; ln++) {
              const lineObj = doc.line(ln);
              entries.push({
                from: lineObj.from,
                to: lineObj.from,
                value: Decoration.line({ class: "cm-diff-draft-line" }),
              });
            }
            const anchor = doc.line(toLine);
            entries.push({
              from: anchor.to,
              to: anchor.to,
              value: Decoration.widget({
                widget: new PortalBlockWidget("draft:active"),
                block: true,
                side: 1,
              }),
            });
          }
        }
        entries.sort((a, b) => a.from - b.from);
        const builder = new RangeSetBuilder<ReturnType<typeof Decoration.line>>();
        for (const item of entries) {
          builder.add(item.from, item.to, item.value);
        }
        return builder.finish();
      };

      const makeSideExtensions = (paneSide: DiffCommentSide) => {
        const decosField = StateField.define({
          create: (s) => buildDecorationsForSide(s.doc, paneSide),
          update: (decos, tr) => {
            if (tr.docChanged || tr.effects.some((e) => e.is(rebuildEffect))) {
              return buildDecorationsForSide(tr.state.doc, paneSide);
            }
            return decos;
          },
          provide: (f) => EditorView.decorations.from(f),
        });

        const reviewExts = reviewScope
          ? [
              decosField,
              gutter({
                class: "cm-diff-comment-gutter",
                lineMarker: (editorView, line) => {
                  if (editorView.state.doc.length === 0 && line.from === 0 && paneSide === "original" && !diff.original) {
                    return null;
                  }
                  const ln = editorView.state.doc.lineAt(line.from).number;
                  return new AddCommentMarker(ln);
                },
                domEventHandlers: {
                  mousedown: (editorView, line, event) => {
                    const mouse = event as MouseEvent;
                    if (mouse.button !== 0) return false;
                    mouse.preventDefault();
                    const clickedLine = editorView.state.doc.lineAt(line.from).number;
                    const currentDraft = draftRef.current;
                    if (mouse.shiftKey && currentDraft && currentDraft.side === paneSide && !currentDraft.editingId) {
                      const startLine = Math.min(currentDraft.startLine, clickedLine);
                      const endLine = Math.max(currentDraft.endLine, clickedLine);
                      setDraft({ ...currentDraft, startLine, endLine });
                      return true;
                    }
                    const sel = editorView.state.selection.main;
                    if (!sel.empty) {
                      const selStart = editorView.state.doc.lineAt(sel.from).number;
                      const selEnd = editorView.state.doc.lineAt(Math.max(sel.from, sel.to - 1)).number;
                      if (clickedLine >= selStart && clickedLine <= selEnd) {
                        setDraft({
                          side: paneSide,
                          startLine: selStart,
                          endLine: selEnd,
                          initialBody: "",
                        });
                        return true;
                      }
                    }
                    setDraft({
                      side: paneSide,
                      startLine: clickedLine,
                      endLine: clickedLine,
                      initialBody: "",
                    });
                    return true;
                  },
                },
              }),
              EditorView.updateListener.of((update) => {
                if (!update.selectionSet) return;
                const sel = update.state.selection.main;
                if (sel.empty) {
                  if (update.view.hasFocus) onSelectionRef.current?.(null);
                  return;
                }
                const startLine = update.state.doc.lineAt(sel.from).number;
                const endLine = update.state.doc.lineAt(Math.max(sel.from, sel.to - 1)).number;
                onSelectionRef.current?.({ side: paneSide, startLine, endLine });
              }),
            ]
          : [];

        const core = [
          lineNumbers(),
          ...reviewExts,
          EditorView.editable.of(false),
          EditorState.readOnly.of(true),
          EditorState.phrases.of({ "$ unchanged lines": unchangedLines }),
          EditorView.lineWrapping,
          theme.vlxCmHighlighting(),
        ];
        return langExt ? [...core, langExt] : core;
      };

      mv = new MergeView({
        a: { doc: diff.original, extensions: makeSideExtensions("original") },
        b: { doc: diff.modified, extensions: makeSideExtensions("modified") },
        parent: ref.current,
        collapseUnchanged: options.collapseUnchanged ? { margin: 3, minSize: 4 } : undefined,
        diffConfig: options.ignoreWhitespace
          ? { override: (a, b) => diffIgnoringWhitespace(a, b, charDiff, Change) ?? charDiff(a, b) }
          : undefined,
        gutter: true,
        highlightChanges: !options.ignoreWhitespace,
      });

      syncDecorationsRef.current = () => {
        if (!mv) return;
        mv.a.dispatch({ effects: rebuildEffect.of(null) });
        mv.b.dispatch({ effects: rebuildEffect.of(null) });
      };
    })();
    return () => {
      cancelled = true;
      syncDecorationsRef.current = null;
      mv?.destroy();
    };
  }, [diff, unchangedLines, options.collapseUnchanged, options.ignoreWhitespace, reviewScope, registerHost]);

  const handleSaveDraft = (body: string) => {
    if (!draft || !reviewScope || !diff) return;
    if (draft.editingId) {
      updateComment(reviewScope, draft.editingId, body);
    } else {
      const sourceText = draft.side === "original" ? diff.original : diff.modified;
      const snippet = extractLineSnippet(sourceText, draft.startLine, draft.endLine);
      addComment(reviewScope, {
        path,
        side: draft.side,
        startLine: draft.startLine,
        endLine: draft.endLine,
        snippet,
        body,
      });
    }
    setDraft(null);
  };

  if (err) {
    return (
      <div style={{ padding: 16, fontSize: 12.5, color: "var(--danger, #e05252)", whiteSpace: "pre-wrap" }}>
        {err}
      </div>
    );
  }
  if (!diff) {
    return (
      <div style={{ padding: 16, fontSize: 12.5, color: "var(--text-muted)" }}>
        {t("changes.loadingDiff")}
      </div>
    );
  }
  if (diff.tooLarge) return <div className="changes-message">{t("changes.tooLarge")}</div>;
  if (diff.binary) {
    return (
      <div style={{ padding: 16, fontSize: 12.5, color: "var(--text-muted)" }}>
        {t("changes.binary")}
      </div>
    );
  }

  const draftHost = widgetHosts["draft:active"];

  return (
    <div className="diff-view-shell">
      {reviewScope && draft && !draftHost && (
        <div className="diff-comment-fallback">
          <InlineCommentForm
            draft={draft}
            onSave={handleSaveDraft}
            onCancel={() => setDraft(null)}
          />
        </div>
      )}
      <div ref={ref} style={{ flex: 1, minHeight: 0, overflow: "auto" }} />
      {reviewScope &&
        fileComments.map((c) => {
          if (draft?.editingId === c.id) return null;
          const host = widgetHosts[`comment:${c.id}`];
          const card = (
            <SavedCommentCard
              key={c.id}
              comment={c}
              onEdit={() =>
                setDraft({
                  side: c.side,
                  startLine: c.startLine,
                  endLine: c.endLine,
                  editingId: c.id,
                  initialBody: c.body,
                })
              }
              onRemove={() => removeComment(reviewScope, c.id)}
            />
          );
          return host ? createPortal(card, host) : null;
        })}
      {reviewScope &&
        draft &&
        draftHost &&
        createPortal(
          <InlineCommentForm
            draft={draft}
            onSave={handleSaveDraft}
            onCancel={() => setDraft(null)}
          />,
          draftHost,
        )}
    </div>
  );
}
