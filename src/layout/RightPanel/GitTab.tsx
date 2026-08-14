//! Right-panel Git tab: branch/ahead-behind data and the working tree's per-file changes, extracted from RightPanel.
//! The shared KV key-value row lives in parts.

import { useEffect, useState } from "react";
import { statusMeta } from "../../components/diff/changeStatus";
import { useT } from "../../i18n";
import { gitChangedFiles, type ChangedFile } from "../../ipc/commands";
import { getGitStatus } from "../../ipc/info";
import { type GitStatus as GitStatusType } from "../../types";
import { KV } from "./parts";
/* ===================== Git (branch data + working tree changes) ===================== */

/** A working tree changes from outside the app — an editor saving, `git add`, `git commit`, a branch
 * switch — and none of those raise an event this panel can subscribe to, so the file list is polled.
 * Matches the cadence the info tab uses for process stats. */
const CHANGES_POLL_MS = 3000;

export function GitTab({ path }: { path: string | null }) {
  const t = useT();
  const [status, setStatus] = useState<GitStatusType | null>(null);
  const [changes, setChanges] = useState<ChangedFile[]>([]);

  useEffect(() => {
    if (!path) {
      setStatus(null);
      return;
    }
    let cancelled = false;
    getGitStatus(path)
      .then((s) => !cancelled && setStatus(s))
      .catch(() => !cancelled && setStatus(null));
    return () => {
      cancelled = true;
    };
  }, [path]);

  useEffect(() => {
    if (!path) {
      setChanges([]);
      return;
    }
    let cancelled = false;
    // Outside a repository the command rejects; an empty list is the honest rendering either way.
    const load = () => {
      gitChangedFiles(path)
        .then((f) => !cancelled && setChanges(f))
        .catch(() => !cancelled && setChanges([]));
    };
    load();
    const timer = setInterval(load, CHANGES_POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [path]);

  return (
    <div style={{ flex: 1, minHeight: 0, overflowY: "auto" }}>
      <div className="insp-section">
        <h4>Branch</h4>
        <KV k="current" v={status?.isRepo ? (status.branch ?? "(detached)") : "—"} accent />
        <KV
          k="upstream"
          v={status?.isRepo ? `↑${status.ahead} ↓${status.behind}` : "—"}
        />
      </div>
      <div className="insp-section">
        <h4>
          {t("changes.title")} · {changes.length}
        </h4>
        {changes.length === 0 ? (
          <div className="diffstat">
            <span className="fn" style={{ color: "var(--text-muted)" }}>
              {t("changes.noChanges")}
            </span>
          </div>
        ) : (
          changes.map((c) => {
            const meta = statusMeta(c.status);
            return (
              <div className="diffstat" key={c.path}>
                <span className={"gb gb-" + meta.letter}>{meta.letter}</span>
                <span className="fn">{c.path}</span>
                <span className="n">
                  {/* Binary files have no line counts: numstat reports `-` for both sides. */}
                  {c.binary ? (
                    <span style={{ color: "var(--text-faint)" }}>—</span>
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
