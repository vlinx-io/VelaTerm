//! Read-only, side-by-side diff of one file in a CodeMirror MergeView, shared by the Changes dialog
//! and the diff document tab. Compares the worktree, one commit, or a commit range, depending on props.

import { useEffect, useRef, useState } from "react";
import type { MergeView } from "@codemirror/merge";
import { useT } from "../../i18n";
import { diffIgnoringWhitespace } from "./whitespaceDiff";
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
}: {
  cwd: string;
  path: string;
  tick: number;
  options: DiffOptions;
  commit: string | null;
  /** Left side of a range comparison; the commit's parent when null. */
  base: string | null;
  side: GitChangeSide;
}) {
  const t = useT();
  const unchangedLines = t("changes.unchangedLines");
  const ref = useRef<HTMLDivElement>(null);
  const [diff, setDiff] = useState<FileDiff | null>(null);
  const [err, setErr] = useState("");

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
    const el = ref.current;
    if (!el || !diff || diff.binary || diff.tooLarge) return;
    let mv: MergeView | null = null;
    let cancelled = false;
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
      const { EditorView, lineNumbers } = view;
      const { EditorState } = state;
      const langExt = await theme.languageExtensionFor(diff.path);
      if (cancelled || !ref.current) return;
      const base = [
        lineNumbers(),
        EditorView.editable.of(false),
        EditorState.readOnly.of(true),
        EditorState.phrases.of({ "$ unchanged lines": unchangedLines }),
        EditorView.lineWrapping,
        theme.vlxCmHighlighting(),
      ];
      const side = langExt ? [...base, langExt] : base;
      mv = new MergeView({
        a: { doc: diff.original, extensions: side },
        b: { doc: diff.modified, extensions: side },
        parent: ref.current,
        collapseUnchanged: options.collapseUnchanged ? { margin: 3, minSize: 4 } : undefined,
        diffConfig: options.ignoreWhitespace
          ? { override: (a, b) => diffIgnoringWhitespace(a, b, charDiff, Change) ?? charDiff(a, b) }
          : undefined,
        gutter: true,
        highlightChanges: !options.ignoreWhitespace,
      });
    })();
    return () => {
      cancelled = true;
      mv?.destroy();
    };
  }, [diff, unchangedLines, options.collapseUnchanged, options.ignoreWhitespace]);

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
  return <div ref={ref} style={{ height: "100%", overflow: "auto" }} />;
}
