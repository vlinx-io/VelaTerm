//! Right-panel Git tab: branch/ahead-behind plus the per-file change list for the focused working directory.
//! Both come from the backend on the same refresh tick. Git has no change notification of its own, so the tab
//! polls while it is on screen, refreshes when the window regains focus, and offers a manual refresh button.
//! The shared KV key-value row lives in parts.

import { useCallback, useEffect, useState } from "react";
import { useT } from "../../i18n";
import { getGitStatus } from "../../ipc/info";
import { gitChangedFiles, type ChangedFile } from "../../ipc/commands";
import { statusLetter } from "../../components/diff/changeStatus";
import { type GitStatus as GitStatusType } from "../../types";
import { KV } from "./parts";

/** Poll interval while the tab is visible. Git exposes no change events, so this is the refresh floor. */
const POLL_MS = 5000;

export function GitTab({ path }: { path: string | null }) {
  const t = useT();
  const [status, setStatus] = useState<GitStatusType | null>(null);
  const [files, setFiles] = useState<ChangedFile[] | null>(null);
  const [err, setErr] = useState("");
  const [tick, setTick] = useState(0);

  const refresh = useCallback(() => setTick((n) => n + 1), []);

  // Switching working directory clears the old repository's data; a poll tick keeps it to avoid flicker.
  useEffect(() => {
    setStatus(null);
    setFiles(null);
    setErr("");
  }, [path]);

  useEffect(() => {
    if (!path) return;
    let cancelled = false;
    void Promise.allSettled([getGitStatus(path), gitChangedFiles(path)]).then(
      ([s, f]) => {
        if (cancelled) return;
        const st = s.status === "fulfilled" ? s.value : null;
        setStatus(st);
        if (f.status === "fulfilled") {
          setFiles(f.value);
          setErr("");
        } else {
          setFiles(null);
          // Outside a repository the changed-files call always fails; the branch row already says so.
          setErr(st && !st.isRepo ? "" : String(f.reason));
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, [path, tick]);

  useEffect(() => {
    if (!path) return;
    const refreshIfVisible = () => {
      if (!document.hidden) refresh();
    };
    const id = setInterval(refreshIfVisible, POLL_MS);
    window.addEventListener("focus", refreshIfVisible);
    document.addEventListener("visibilitychange", refreshIfVisible);
    return () => {
      clearInterval(id);
      window.removeEventListener("focus", refreshIfVisible);
      document.removeEventListener("visibilitychange", refreshIfVisible);
    };
  }, [path, refresh]);

  const isRepo = status?.isRepo ?? false;

  return (
    <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
      <div className="insp-section">
        <h4>{t("info.branch")}</h4>
        <KV k="current" v={isRepo ? (status?.branch ?? "(detached)") : "—"} accent />
        <KV k="upstream" v={isRepo ? `↑${status?.ahead} ↓${status?.behind}` : "—"} />
      </div>
      <div className="insp-section">
        <h4 style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <span style={{ flex: 1 }}>
            {t("changes.title")}
            {files ? ` · ${files.length}` : ""}
          </span>
          <button
            type="button"
            className="insp-refresh"
            title={t("changes.refresh")}
            aria-label={t("changes.refresh")}
            onClick={refresh}
          >
            ⟳
          </button>
        </h4>
        {!isRepo && !err ? (
          <div className="insp-empty">{t("changes.notRepo")}</div>
        ) : err ? (
          <div className="insp-empty" style={{ color: "var(--danger, #e05252)" }}>
            {err}
          </div>
        ) : !files ? (
          <div className="insp-empty">{t("changes.loading")}</div>
        ) : files.length === 0 ? (
          <div className="insp-empty">{t("changes.noChanges")}</div>
        ) : (
          files.map((c) => {
            const letter = statusLetter(c.status);
            return (
              <div className="diffstat" key={c.path} title={c.path}>
                <span className={"gb gb-" + letter}>{letter}</span>
                <span className="fn">{c.path}</span>
                <span className="n">
                  {c.binary ? (
                    <span style={{ color: "var(--text-faint)" }}>bin</span>
                  ) : (
                    <>
                      <span style={{ color: "var(--green)" }}>+{c.additions}</span>{" "}
                      <span style={{ color: "var(--red)" }}>−{c.deletions}</span>
                    </>
                  )}
                </span>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
