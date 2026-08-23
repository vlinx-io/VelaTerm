//! Commit history for the Git panel: a page of commits at a time, each expandable into the files it
//! touched. Clicking a file opens the Changes modal in commit mode, so committed and uncommitted
//! diffs are read in the same viewer.
//!
//! History is loaded on demand rather than with the rest of the tab: a repository can hold hundreds
//! of thousands of commits, and the tab polls every few seconds.

import { useCallback, useEffect, useState } from "react";
import Icons from "../../../components/Icons";
import { statusLetter } from "../../../components/diff/changeStatus";
import { useT } from "../../../i18n";
import {
  gitCommitFiles,
  gitLogPage,
  type ChangedFile,
  type CommitInfo,
} from "../../../ipc/commands";
import { useTermStore } from "../../../store/termStore";

/** Commits fetched per page. Enough to fill the panel without holding a whole history in memory. */
const PAGE = 30;

/** Files of one commit, loaded the first time that commit is expanded. */
function CommitFiles({ cwd, hash }: { cwd: string; hash: string }) {
  const t = useT();
  const [files, setFiles] = useState<ChangedFile[] | null>(null);
  const [err, setErr] = useState("");
  const openChanges = useTermStore((s) => s.openChanges);

  useEffect(() => {
    let alive = true;
    void gitCommitFiles(cwd, hash)
      .then((f) => {
        if (alive) setFiles(f);
      })
      .catch((e) => {
        if (alive) setErr(String(e));
      });
    return () => {
      alive = false;
    };
  }, [cwd, hash]);

  if (err) return <div className="insp-empty git-err">{err}</div>;
  if (!files) return <div className="insp-empty">{t("changes.loading")}</div>;
  // A merge commit has no diff against a single parent, so there is nothing to list.
  if (files.length === 0) return <div className="insp-empty">{t("git.commitNoFiles")}</div>;

  return (
    <div className="git-commit-files">
      {files.map((f) => {
        const letter = statusLetter(f.status);
        return (
          <button
            type="button"
            className="git-row-main"
            key={f.path}
            title={f.path}
            onClick={() => openChanges(cwd, { path: f.path, commit: hash })}
          >
            <span className={"gb gb-" + letter}>{letter}</span>
            <span className="nm">
              <span className="base">{f.path}</span>
            </span>
            {!f.binary && (
              <span className="n">
                <span className="plus">+{f.additions}</span>{" "}
                <span className="minus">−{f.deletions}</span>
              </span>
            )}
          </button>
        );
      })}
    </div>
  );
}

export function CommitHistory({
  cwd,
  total,
  tick,
}: {
  cwd: string;
  total: number;
  /** Panel refresh counter. A new commit shifts every offset, so paged rows must be reloaded, not appended. */
  tick: number;
}) {
  const t = useT();
  const [commits, setCommits] = useState<CommitInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState("");
  const [expanded, setExpanded] = useState<string | null>(null);

  const loadPage = useCallback(
    (offset: number) => {
      setLoading(true);
      gitLogPage(cwd, PAGE, offset)
        .then((page) => {
          // Replacing at offset 0 and appending otherwise keeps a refresh from duplicating rows.
          setCommits((cur) => (offset === 0 ? page : [...cur, ...page]));
          setErr("");
        })
        .catch((e) => setErr(String(e)))
        .finally(() => setLoading(false));
    },
    [cwd],
  );

  // Load the first page when the section mounts, and start over when the repository changes or the
  // panel refreshes. Committing shifts the whole history by one, so keeping the old rows would both
  // hide the new commit and make "Load more" re-append a row that is already on screen.
  useEffect(() => {
    setCommits([]);
    setExpanded(null);
    loadPage(0);
  }, [cwd, loadPage, tick]);

  if (err) return <div className="insp-empty git-err">{err}</div>;
  if (commits.length === 0) {
    return <div className="insp-empty">{loading ? t("changes.loading") : t("git.noCommits")}</div>;
  }

  return (
    <div>
      {commits.map((c) => {
        const open = expanded === c.hash;
        return (
          <div key={c.hash} className="git-commit-item">
            <button
              type="button"
              className="git-commit-row"
              onClick={() => setExpanded(open ? null : c.hash)}
              aria-expanded={open}
              title={`${c.hash} · ${c.author}`}
            >
              <span className="tw">
                {open ? <Icons.chevD size={11} /> : <Icons.chevR size={11} />}
              </span>
              <span className="hash">{c.hash}</span>
              <span className="subj">{c.subject}</span>
              <span className="when">{c.relative}</span>
            </button>
            {open && <CommitFiles cwd={cwd} hash={c.hash} />}
          </div>
        );
      })}
      {commits.length < total && (
        <button
          type="button"
          className="git-more"
          disabled={loading}
          onClick={() => loadPage(commits.length)}
        >
          {loading ? t("changes.loading") : t("git.loadMore")}
        </button>
      )}
    </div>
  );
}
