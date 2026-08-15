//! Changes modal showing all uncommitted workspace changes relative to HEAD: staged, unstaged, and untracked.
//! The left side lists changed files; selecting one renders a line-by-line HEAD-versus-worktree diff in a
//! CodeMirror MergeView on the right. Mounted at the App root and driven by changesCwd in the store, this
//! self-contained overlay does not interact with center tabs or keep-alive behavior.

import { useEffect, useRef, useState } from "react";
import { MergeView } from "@codemirror/merge";
import { EditorView, lineNumbers } from "@codemirror/view";
import { EditorState } from "@codemirror/state";
import { useT } from "../../i18n";
import { useSuspendNativeViews } from "../../hooks/nativeViewSuspend";
import {
  gitChangedFiles,
  gitFileDiff,
  type ChangedFile,
  type FileDiff,
} from "../../ipc/commands";
import { useTermStore } from "../../store/termStore";
import { STATUS_META } from "./changeStatus";
import { languageExtensionFor, vlxCmHighlighting } from "./codeMirrorTheme";

/**
 * Render a single-file diff by loading both texts into a read-only MergeView, adding language support
 * transparently once loaded. `tick` is the modal's refresh counter: it carries no data, it only forces
 * the open diff to reload so it cannot disagree with the refreshed line counts beside it.
 */
function DiffView({ cwd, path, tick }: { cwd: string; path: string; tick: number }) {
  const t = useT();
  const ref = useRef<HTMLDivElement>(null);
  const [diff, setDiff] = useState<FileDiff | null>(null);
  const [err, setErr] = useState("");

  useEffect(() => {
    let alive = true;
    setDiff(null);
    setErr("");
    void gitFileDiff(cwd, path)
      .then((d) => {
        if (alive) setDiff(d);
      })
      .catch((e) => {
        if (alive) setErr(String(e));
      });
    return () => {
      alive = false;
    };
  }, [cwd, path, tick]);

  useEffect(() => {
    const el = ref.current;
    if (!el || !diff || diff.binary) return;
    let mv: MergeView | null = null;
    let cancelled = false;
    void (async () => {
      const langExt = await languageExtensionFor(diff.path);
      if (cancelled || !ref.current) return;
      const base = [
        lineNumbers(),
        EditorView.editable.of(false),
        EditorState.readOnly.of(true),
        EditorView.lineWrapping,
        vlxCmHighlighting(),
      ];
      const side = langExt ? [...base, langExt] : base;
      mv = new MergeView({
        a: { doc: diff.original, extensions: side },
        b: { doc: diff.modified, extensions: side },
        parent: ref.current,
        collapseUnchanged: { margin: 3, minSize: 4 },
        gutter: true,
        highlightChanges: true,
      });
    })();
    return () => {
      cancelled = true;
      mv?.destroy();
    };
  }, [diff]);

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
  if (diff.binary) {
    return (
      <div style={{ padding: 16, fontSize: 12.5, color: "var(--text-muted)" }}>
        {t("changes.binary")}
      </div>
    );
  }
  return <div ref={ref} style={{ height: "100%", overflow: "auto" }} />;
}

export function ChangesModal() {
  const t = useT();
  const cwd = useTermStore((s) => s.changesCwd);
  const close = useTermStore((s) => s.closeChanges);
  const [files, setFiles] = useState<ChangedFile[] | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [err, setErr] = useState("");
  const [tick, setTick] = useState(0);

  // Opening a different directory resets the list; a manual refresh keeps the current selection instead.
  useEffect(() => {
    setFiles(null);
    setSelected(null);
    setErr("");
  }, [cwd]);

  useEffect(() => {
    if (!cwd) return;
    let alive = true;
    void gitChangedFiles(cwd)
      .then((fs) => {
        if (!alive) return;
        setFiles(fs);
        setSelected((cur) =>
          cur && fs.some((f) => f.path === cur) ? cur : (fs[0]?.path ?? null),
        );
        setErr("");
      })
      .catch((e) => {
        if (alive) setErr(String(e));
      });
    return () => {
      alive = false;
    };
  }, [cwd, tick]);

  // Suspend native browser views while the modal is visible so they cannot cover it (architecture document §17).
  useSuspendNativeViews(Boolean(cwd));

  if (!cwd) return null;

  return (
    <div
      role="dialog"
      aria-label={t("changes.title")}
      onMouseDown={() => close()}
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 1200,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: "rgba(0,0,0,0.45)",
      }}
    >
      <div
        onMouseDown={(e) => e.stopPropagation()}
        style={{
          width: "min(1040px, calc(100vw - 48px))",
          height: "min(720px, calc(100vh - 64px))",
          display: "flex",
          flexDirection: "column",
          background: "var(--bg-panel)",
          border: "1px solid var(--border-strong)",
          borderRadius: 10,
          boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
          overflow: "hidden",
        }}
      >
        {/* Header. */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "12px 16px",
            borderBottom: "1px solid var(--border)",
          }}
        >
          <div style={{ fontSize: 13.5, fontWeight: 600, color: "var(--text-primary)" }}>
            {t("changes.title")}
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <button className="vlx-btn" onClick={() => setTick((n) => n + 1)}>
              {t("changes.refresh")}
            </button>
            <button className="vlx-btn" onClick={close}>
              {t("common.close")}
            </button>
          </div>
        </div>

        {/* Body: file list on the left, diff on the right. */}
        <div style={{ flex: 1, display: "flex", minHeight: 0 }}>
          <div
            style={{
              width: 280,
              flex: "none",
              borderRight: "1px solid var(--border)",
              overflowY: "auto",
              padding: 6,
            }}
          >
            {err && (
              <div style={{ padding: 10, fontSize: 12, color: "var(--danger, #e05252)", whiteSpace: "pre-wrap" }}>
                {err}
              </div>
            )}
            {!err && files === null && (
              <div style={{ padding: 10, fontSize: 12, color: "var(--text-muted)" }}>
                {t("changes.loading")}
              </div>
            )}
            {!err && files !== null && files.length === 0 && (
              <div style={{ padding: 10, fontSize: 12, color: "var(--text-muted)" }}>
                {t("changes.noChanges")}
              </div>
            )}
            {files?.map((f) => {
              const meta = STATUS_META[f.status] ?? STATUS_META.modified;
              const active = f.path === selected;
              // Split into directory and filename. The directory may truncate with an ellipsis, while the filename
              // remains complete to preserve the most informative tail. Backend paths always use forward slashes
              // (see ChangedFile.path), so splitting on the final "/" is sufficient.
              const slash = f.path.lastIndexOf("/");
              const dir = slash >= 0 ? f.path.slice(0, slash + 1) : "";
              const base = slash >= 0 ? f.path.slice(slash + 1) : f.path;
              return (
                <div
                  key={f.path}
                  onClick={() => setSelected(f.path)}
                  title={f.path}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 8,
                    padding: "5px 8px",
                    borderRadius: 5,
                    cursor: "pointer",
                    background: active ? "var(--bg-hover)" : "transparent",
                  }}
                >
                  <span
                    style={{ flex: "none", width: 12, textAlign: "center", fontFamily: "var(--font-mono)", fontSize: 11, fontWeight: 700, color: meta.color }}
                  >
                    {meta.letter}
                  </span>
                  {/* DO NOT go back to direction:rtl. An early version used it to ellipsize from the
                      left and keep the trailing file name visible, but that bidi trick swallows the
                      first character of a purely LTR name under WebKit (Tauri's WKWebView) — for
                      example `java-app-samples` renders as `ava-app-samples`. The directory segment
                      now shrinks and truncates through flex while the file name is always shown,
                      which depends on no bidi behaviour, is identical on all three platforms, and
                      still keeps the trailing file name visible as originally intended. */}
                  <span
                    style={{
                      flex: 1,
                      minWidth: 0,
                      display: "flex",
                      overflow: "hidden",
                      fontSize: 12,
                      color: "var(--text-primary)",
                    }}
                  >
                    {dir && (
                      <span
                        style={{
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                          color: "var(--text-muted)",
                        }}
                      >
                        {dir}
                      </span>
                    )}
                    <span style={{ flex: "none", whiteSpace: "nowrap" }}>{base}</span>
                  </span>
                  {!f.binary && (f.additions > 0 || f.deletions > 0) && (
                    <span style={{ flex: "none", fontSize: 10.5, fontFamily: "var(--font-mono)" }}>
                      <span style={{ color: "var(--green, #3fb950)" }}>+{f.additions}</span>{" "}
                      <span style={{ color: "var(--danger, #e05252)" }}>-{f.deletions}</span>
                    </span>
                  )}
                </div>
              );
            })}
          </div>

          <div style={{ flex: 1, minWidth: 0, overflow: "hidden" }}>
            {selected ? (
              <DiffView cwd={cwd} path={selected} tick={tick} />
            ) : (
              <div style={{ padding: 16, fontSize: 12.5, color: "var(--text-muted)" }}>
                {t("changes.selectFile")}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
