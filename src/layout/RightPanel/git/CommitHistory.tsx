//! Commit history for the Git panel: a page of commits at a time, each expandable into the files it
//! touched. Clicking a file opens the Changes modal in commit mode, so committed and uncommitted
//! diffs are read in the same viewer.
//!
//! History is loaded on demand rather than with the rest of the tab: a repository can hold hundreds
//! of thousands of commits, and the tab polls every few seconds.

import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
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
import { layoutGraph, parseRefs, type GraphRow } from "./gitGraph";

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
  // The backend compares against the first parent, including merge commits.
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

/** Full object identity stays stable when Git changes its abbreviation length. */
const commitId = (commit: CommitInfo) => commit.oid ?? commit.hash;

/** Graph geometry, in pixels: one lane per column, one row per commit line. */
const LANE = 10;
const ROW = 24;
const MID = ROW / 2;
/** Lanes drawn at most; wider histories clip on the right rather than squeezing the subject. */
const MAX_LANES = 8;
const laneX = (i: number) => i * LANE + LANE / 2;

/** One row of the history graph: lines through, into, and out of this commit's node. */
function GraphCell({ row, lanes, merge }: { row: GraphRow; lanes: number; merge: boolean }) {
  const x = laneX(row.col);
  const curve = (x1: number, y1: number, x2: number, y2: number) =>
    x1 === x2 ? `M${x1} ${y1}V${y2}` : `M${x1} ${y1}C${x1} ${(y1 + y2) / 2} ${x2} ${(y1 + y2) / 2} ${x2} ${y2}`;
  return (
    <svg className="git-graph" width={lanes * LANE} height={ROW} preserveAspectRatio="none" viewBox={`0 0 ${lanes * LANE} ${ROW}`} aria-hidden="true">
      {row.through.map((i) => <path key={"t" + i} className={"lane-" + (i % 6)} d={`M${laneX(i)} 0V${ROW}`} />)}
      {row.incoming.map((i) => <path key={"i" + i} className={"lane-" + (i % 6)} d={curve(laneX(i), 0, x, MID)} />)}
      {row.outgoing.map((i) => <path key={"o" + i} className={"lane-" + (i % 6)} d={curve(x, MID, laneX(i), ROW)} />)}
      <circle className={"node lane-" + row.color + (merge ? " merge" : "")} cx={x} cy={MID} r={3.5} />
    </svg>
  );
}

/** Lanes still running below a commit, drawn beside its expanded file list so the lines continue. */
function GraphContinuation({ row, lanes }: { row: GraphRow; lanes: number }) {
  const live = [...new Set([...row.through, ...row.outgoing])].filter((i) => i < lanes);
  return (
    <svg className="git-graph git-graph-cont" width={lanes * LANE} preserveAspectRatio="none" aria-hidden="true">
      {live.map((i) => <line key={i} className={"lane-" + (i % 6)} x1={laneX(i)} x2={laneX(i)} y1="0" y2="100%" />)}
    </svg>
  );
}

export function CommitHistory({ cwd, total, head, refreshToken = 0, onContextMenu }: {
  cwd: string;
  total: number;
  head: string | null;
  refreshToken?: number;
  /** Right-click on a commit row, for copy and agent actions. */
  onContextMenu?: (commit: CommitInfo, e: React.MouseEvent) => void;
}) {
  const t = useT();
  const [commits, setCommits] = useState<CommitInfo[]>([]);
  const [limit, setLimit] = useState(PAGE);
  const [loading, setLoading] = useState(false);
  const [err, setErr] = useState("");
  const [expanded, setExpanded] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const rowsRef = useRef<{ cwd: string; rows: CommitInfo[] }>({ cwd, rows: [] });
  const containerRef = useRef<HTMLDivElement>(null);
  const anchorRef = useRef<{ parent: HTMLElement; oid: string; offset: number } | null>(null);

  useEffect(() => {
    setCommits([]);
    setExpanded(null);
    setLimit(PAGE);
    rowsRef.current = { cwd, rows: [] };
    anchorRef.current = null;
  }, [cwd]);

  useEffect(() => {
    let cancelled = false;
    const previous = rowsRef.current.cwd === cwd ? rowsRef.current.rows : [];
    setLoading(true);
    void (async () => {
      let wanted = Math.max(limit, previous.length);
      const rows: CommitInfo[] = [];
      while (rows.length < wanted) {
        const size = Math.min(200, wanted - rows.length);
        const page = await gitLogPage(cwd, size, rows.length, head);
        if (cancelled) return;
        rows.push(...page);
        // Preserve the old loaded range when commits are prepended to its same history.
        if (previous.length) {
          const shift = rows.findIndex((row) => commitId(row) === commitId(previous[0]));
          if (shift >= 0) wanted = Math.max(wanted, previous.length + shift);
        }
        if (page.length < size) break;
      }
      if (cancelled) return;
      const unique = [...new Map(rows.map((row) => [commitId(row), row])).values()];
      const parent = containerRef.current?.closest<HTMLElement>("[data-git-scroll]");
      if (parent) {
        const top = parent.getBoundingClientRect().top;
        const bottom = parent.getBoundingClientRect().bottom;
        const visible = Array.from(containerRef.current!.querySelectorAll<HTMLElement>("[data-commit-oid]"))
          .find((row) => row.getBoundingClientRect().bottom > top && row.getBoundingClientRect().top < bottom);
        if (visible?.dataset.commitOid) anchorRef.current = {
          parent, oid: visible.dataset.commitOid, offset: visible.getBoundingClientRect().top - top,
        };
      }
      rowsRef.current = { cwd, rows: unique };
      setCommits(unique);
      setExpanded((current) => current && unique.some((row) => commitId(row) === current) ? current : null);
      setErr("");
    })().catch((e) => { if (!cancelled) setErr(String(e)); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [cwd, head, limit, refreshToken, retry]);

  // Retry transient failures only after the previous request finishes, preserving readable rows.
  useEffect(() => {
    if (!err || loading || document.hidden) return;
    const timer = setTimeout(() => setRetry((n) => n + 1), 5000);
    return () => clearTimeout(timer);
  }, [err, loading, retry]);

  // The graph needs parents for every commit; an older server without them gets the plain list.
  const graph = useMemo(() => commits.every((c) => c.parents)
    ? layoutGraph(commits.map((c) => ({ oid: commitId(c), parents: c.parents ?? [] }))) : null, [commits]);
  const lanes = graph ? Math.min(MAX_LANES, Math.max(1, ...graph.map((r) => r.width))) : 0;

  useLayoutEffect(() => {
    const anchor = anchorRef.current;
    anchorRef.current = null;
    if (!anchor) return;
    const row = Array.from(containerRef.current?.querySelectorAll<HTMLElement>("[data-commit-oid]") ?? [])
      .find((item) => item.dataset.commitOid === anchor.oid);
    if (row) anchor.parent.scrollTop += row.getBoundingClientRect().top
      - anchor.parent.getBoundingClientRect().top - anchor.offset;
  }, [commits]);

  return (
    <div ref={containerRef} aria-busy={loading}>
      {err && <div className="insp-empty git-err" role="status">{err}</div>}
      {commits.length === 0 && !err && (
        <div className="insp-empty">{loading ? t("changes.loading") : t("git.noCommits")}</div>
      )}
      {commits.map((c, index) => {
        const oid = commitId(c);
        const open = expanded === oid;
        const row = graph?.[index];
        const refs = parseRefs(c.refs);
        return (
          <div key={oid} data-commit-oid={oid} className={"git-commit-item" + (row ? " graphed" : "")}>
            <button
              type="button"
              className="git-commit-row"
              onClick={() => setExpanded(open ? null : oid)}
              onContextMenu={onContextMenu ? (e) => onContextMenu(c, e) : undefined}
              aria-expanded={open}
              title={`${c.message ?? c.subject}\n\n${c.hash} · ${c.author}`}
            >
              <span className="tw">
                {open ? <Icons.chevD size={11} /> : <Icons.chevR size={11} />}
              </span>
              {row && <GraphCell row={row} lanes={lanes} merge={(c.parents?.length ?? 0) > 1} />}
              <span className="hash">{c.hash}</span>
              {/* One label keeps the subject readable in a narrow panel; the rest are one hover away. */}
              {refs.slice(0, 1).map((r) => (
                <span key={r.kind + r.name} className={"git-ref git-ref-" + r.kind} title={r.name}>{r.name}</span>
              ))}
              {refs.length > 1 && (
                <span className="git-ref git-ref-more" title={refs.slice(1).map((r) => r.name).join("\n")}>+{refs.length - 1}</span>
              )}
              <span className="subj">{c.subject}</span>
              <span className="when">{c.relative}</span>
            </button>
            {open && (
              <div className="git-commit-expanded" style={row ? { paddingLeft: lanes * LANE } : undefined}>
                {row && <GraphContinuation row={row} lanes={lanes} />}
                <CommitFiles cwd={cwd} hash={oid} />
              </div>
            )}
          </div>
        );
      })}
      {commits.length > 0 && commits.length < total && (
        <button
          type="button"
          className="git-more"
          disabled={loading}
          onClick={() => setLimit(Math.max(limit, commits.length) + PAGE)}
        >
          {loading ? t("changes.loading") : t("git.loadMore")}
        </button>
      )}
    </div>
  );
}
