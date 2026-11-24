//! Changes tab showing one comparison: the staged, unstaged, or combined worktree changes, one commit, or a
//! commit range. The left side lists changed files; selecting one renders a line-by-line diff in a CodeMirror
//! MergeView on the right, with inline review comments that can be batched and sent back to the session's agent.

import { useEffect, useMemo, useState } from "react";
import Icons from "../Icons";
import { useT } from "../../i18n";
import {
  FileDiffView as DiffView,
  readDiffOptions,
  writeDiffOptions,
  type DiffDraftRequest,
  type DiffOptions,
  type DiffSelectionRange,
} from "./FileDiffView";
import { DiffReviewBar } from "./DiffReviewBar";
import { formatLineRange, reviewScopeKey, useScopeComments } from "./diffReviewStore";
import { filesForSide } from "./changeGroups";
import { changesTitle } from "./changesTitle";
import { GitFileRow } from "../../layout/RightPanel/git/GitFileRow";
import "./changes-modal.css";
import "../../layout/CenterPane/doc/docTheme.css";
import {
  gitChangedFiles,
  gitCommitFiles,
  gitRangeFiles,
  type ChangedFile,
} from "../../ipc/commands";
import { useTermStore, type DocTab } from "../../store/termStore";

export function ChangesView({ tab, hidden }: { tab: DocTab; hidden: boolean }) {
  const t = useT();
  const select = useTermStore((s) => s.selectChangesFile);
  const refresh = useTermStore((s) => s.refreshDocTab);
  const target = tab.changes!;
  const { cwd, side, commit } = target;
  const range = commit ? target.base : null;
  const base = range?.from ?? null;
  const wanted = target.path;
  // The tab's refresh action and a repeated open bump reloadNonce; both reread the list and the diff.
  const tick = tab.reloadNonce;
  const [files, setFiles] = useState<ChangedFile[] | null>(null);
  const [err, setErr] = useState("");
  const [options, setOptionsState] = useState<DiffOptions>(readDiffOptions);
  const [lineSelection, setLineSelection] = useState<DiffSelectionRange | null>(null);
  const [draftRequest, setDraftRequest] = useState<DiffDraftRequest | null>(null);

  const scopeKey = useMemo(
    () => reviewScopeKey({ cwd, side, commit, base: range }),
    [cwd, side, commit, range],
  );
  const comments = useScopeComments(scopeKey);
  const commentCountsByFile = useMemo(() => {
    const map = new Map<string, number>();
    for (const c of comments) map.set(c.path, (map.get(c.path) ?? 0) + 1);
    return map;
  }, [comments]);

  const setOption = (key: keyof DiffOptions) => setOptionsState((current) => {
    const next = { ...current, [key]: !current[key] };
    writeDiffOptions(next);
    return next;
  });

  useEffect(() => {
    let alive = true;
    const load = commit && base ? gitRangeFiles(cwd, base, commit)
      : commit ? gitCommitFiles(cwd, commit) : gitChangedFiles(cwd).then((list) => list.files);
    void load.then((all) => {
      if (alive) { setFiles(commit ? all : filesForSide(all, side)); setErr(""); }
    }).catch((e) => { if (alive) setErr(String(e)); });
    return () => { alive = false; };
  }, [cwd, commit, base, side, tick]);

  // A requested file that is not in the list, such as one committed since, gives way to the first file.
  const selected = files ? (wanted && files.some((f) => f.path === wanted) ? wanted : files[0]?.path ?? null) : wanted;
  useEffect(() => {
    if (files && selected !== wanted) select(tab.id, selected);
  }, [files, selected, wanted, select, tab.id]);

  const file = files?.find((f) => f.path === selected);
  const comparison = base && range ? t("changes.compareBranch", range.label) : commit ? t("changes.compareCommit") : side === "staged" ? t("changes.compareStaged")
    : side === "unstaged" || side === "untracked" ? t("changes.compareUnstaged") : t("changes.compareAll");
  const title = changesTitle(t, target);

  const triggerComment = () => {
    const sel = lineSelection ?? {
      side: file?.status === "deleted" ? "original" : "modified",
      startLine: 1,
      endLine: 1,
    };
    setDraftRequest((prev) => ({
      nonce: (prev?.nonce ?? 0) + 1,
      side: sel.side,
      startLine: sel.startLine,
      endLine: sel.endLine,
    }));
  };

  return (
    <div className="docview" style={hidden ? { display: "none" } : undefined} aria-label={title}>
      <div className="docview-head">
        <span className="title">{title}</span>
        <span className="path" title={cwd}>{cwd}</span>
        <span className="changes-comparison docview-diff-comparison">{comparison}</span>
        <button type="button" className="vlx-btn changes-toggle" aria-pressed={options.ignoreWhitespace}
          onClick={() => setOption("ignoreWhitespace")}>{t("changes.ignoreWhitespace")}</button>
        <button type="button" className="vlx-btn changes-toggle" aria-pressed={options.collapseUnchanged}
          onClick={() => setOption("collapseUnchanged")}>{t("changes.collapseUnchanged")}</button>
        <button type="button" className="vlx-btn" onClick={() => refresh(tab.id)}>{t("changes.refresh")}</button>
      </div>
      <div className="changes-stack">
        <DiffReviewBar
          scopeKey={scopeKey}
          cwd={cwd}
          onSelectFile={(path) => select(tab.id, path)}
        />
        <div className="changes-body">
          <div className="changes-files" aria-label={t("git.changedFiles", files?.length ?? 0)}>
            {err && <div className="changes-message git-err" role="alert">{err}</div>}
            {!err && files === null && <div className="changes-message">{t("changes.loading")}</div>}
            {!err && files?.length === 0 && <div className="changes-message">{t("changes.noChanges")}</div>}
            {files?.map((f) => {
              const count = commentCountsByFile.get(f.path) ?? 0;
              return (
                <div key={f.path} className={`changes-file-entry${f.path === selected ? " changes-selected" : ""}`}>
                  <GitFileRow file={f} additions={f.additions} deletions={f.deletions} binary={f.binary}
                    onOpen={() => select(tab.id, f.path)} actions={[]} />
                  {count > 0 && (
                    <span className="changes-comment-badge" aria-label={t("changes.review.count", count, 1)}>
                      {count}
                    </span>
                  )}
                </div>
              );
            })}
          </div>
          <div className="changes-content">
            {selected && files ? <>
              <div className="changes-file-head">
                <div className="changes-file-path">
                  {file?.originalPath && <span>{file.originalPath} → </span>}
                  {selected}
                </div>
                {!file?.binary && (
                  <button
                    type="button"
                    className="vlx-btn changes-add-comment-btn"
                    onClick={triggerComment}
                  >
                    <Icons.plus size={12} aria-hidden="true" />
                    <span>
                      {lineSelection
                        ? t("changes.review.addLine", formatLineRange(lineSelection.startLine, lineSelection.endLine))
                        : t("changes.review.addFile")}
                    </span>
                  </button>
                )}
              </div>
              <div className="changes-editor">
                <DiffView
                  key={`${cwd}:${base}:${commit}:${side}:${selected}`}
                  cwd={cwd}
                  path={selected}
                  tick={tick}
                  commit={commit}
                  base={base}
                  side={side}
                  options={options}
                  reviewScope={scopeKey}
                  draftRequest={draftRequest}
                  onSelectionChange={setLineSelection}
                />
              </div>
            </> : <div className="changes-message">{files ? t("changes.selectFile") : t("changes.loading")}</div>}
          </div>
        </div>
      </div>
    </div>
  );
}
