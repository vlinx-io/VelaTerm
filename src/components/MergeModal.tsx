//! Unified Git branch-merge dialog, replacing the separate merge-to-parent and merge-from-parent
//! flows. Users choose both source and target branches and can swap direction, using real branch
//! names without an invented baseline concept. The merge runs in the target branch's checked-out
//! worktree and is unavailable when the target is not checked out. Dirty source changes are committed
//! first with a required message. Conflicts remain in place for resolution in the target worktree.
//! Mounted at the App root, it reads mergeTarget from the store and supports Git-backed sessions.

import { useEffect, useState } from "react";
import { useT } from "../i18n";
import { useSuspendNativeViews } from "../hooks/nativeViewSuspend";
import {
  gitBranchList,
  gitMergeApply,
  gitMergePreview,
  type BranchList,
  type MergeBranchesPreview,
} from "../ipc/commands";
import { useTermStore } from "../store/termStore";
import Select from "./Select";

type Phase = "loading" | "notRepo" | "ready" | "working" | "done" | "conflict" | "error";

export function MergeModal() {
  const t = useT();
  const target = useTermStore((s) => s.mergeTarget);
  const sessions = useTermStore((s) => s.sessions);
  const groups = useTermStore((s) => s.groups);
  const projects = useTermStore((s) => s.projects);
  const close = useTermStore((s) => s.closeMerge);

  // mergeTarget may identify a session or group; either can own a worktree whose directory is mergeable.
  const session = target ? sessions.find((s) => s.id === target) : undefined;
  const group = target && !session ? groups.find((g) => g.id === target) : undefined;
  const entity = session ?? group;
  // Resolve the repository directory from worktree first, then session cwd, then project root.
  // Groups without a worktree also use the project root, matching the sidebar Git menu.
  const projectRoot = entity
    ? projects.find((p) => p.id === entity.projectId)?.rootPath ?? ""
    : "";
  const repoDir = (
    session?.worktreePath ||
    session?.cwd ||
    group?.worktreePath ||
    projectRoot ||
    ""
  ).trim();

  const [phase, setPhase] = useState<Phase>("loading");
  const [branches, setBranches] = useState<BranchList | null>(null);
  const [source, setSource] = useState("");
  const [targetBranch, setTargetBranch] = useState("");
  const [preview, setPreview] = useState<MergeBranchesPreview | null>(null);
  const [previewLoading, setPreviewLoading] = useState(false);
  const [commitMsg, setCommitMsg] = useState("");
  const [error, setError] = useState("");
  const [conflicts, setConflicts] = useState<string[]>([]);

  // Load branches on open. Default the source to the current branch and require an explicit target.
  useEffect(() => {
    if (!target || !entity) return;
    setPhase("loading");
    setBranches(null);
    setSource("");
    setTargetBranch("");
    setPreview(null);
    setCommitMsg("");
    setError("");
    setConflicts([]);
    if (!repoDir) {
      setPhase("notRepo");
      return;
    }
    let alive = true;
    void gitBranchList(repoDir)
      .then((b) => {
        if (!alive) return;
        if (!b.isRepo) {
          setPhase("notRepo");
          return;
        }
        setBranches(b);
        setSource(b.current ?? "");
        setPhase("ready");
      })
      .catch((e) => {
        if (!alive) return;
        setError(String(e));
        setPhase("error");
      });
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [target]);

  const sameBranch = !!source && source === targetBranch;

  // Once distinct branches are selected, load the diff summary, availability, and both dirty states.
  useEffect(() => {
    setPreview(null);
    if (!repoDir || !source || !targetBranch || sameBranch) return;
    let alive = true;
    setPreviewLoading(true);
    void gitMergePreview(repoDir, source, targetBranch)
      .then((p) => {
        if (!alive) return;
        setPreview(p);
        // For a dirty source worktree, prefill the session/group name as an editable commit message.
        if (p.sourceDirty) {
          setCommitMsg((prev) => (prev.trim() ? prev : (entity?.name ?? "")));
        }
      })
      .catch((e) => {
        if (!alive) return;
        setError(String(e));
        setPhase("error");
      })
      .finally(() => {
        if (alive) setPreviewLoading(false);
      });
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repoDir, source, targetBranch]);

  // Suspend native browser views while visible so they cannot cover the dialog.
  useSuspendNativeViews(Boolean(target && entity));

  if (!target || !entity) return null;

  const canMerge =
    !!preview &&
    preview.available &&
    !preview.upToDate &&
    !sameBranch &&
    // A dirty source worktree requires a nonempty message for its automatic commit.
    (!preview.sourceDirty || commitMsg.trim().length > 0);

  const reasonText = (reason: string): string => {
    switch (reason) {
      case "same_branch":
        return t("merge.sameBranch");
      case "branch_not_found":
        return t("merge.branchGone");
      case "target_not_checked_out":
        return t("merge.targetNotCheckedOut", targetBranch);
      default:
        return "";
    }
  };

  const doMerge = async () => {
    if (!preview) return;
    setPhase("working");
    setError("");
    try {
      const outcome = await gitMergeApply(
        repoDir,
        source,
        targetBranch,
        preview.sourceDirty ? commitMsg.trim() : null,
      );
      if (outcome.conflict) {
        setConflicts(outcome.conflicts);
        setPhase("conflict");
        return;
      }
      if (!outcome.merged) {
        setError(outcome.message);
        setPhase("error");
        return;
      }
      setPhase("done");
    } catch (e) {
      setError(String(e));
      setPhase("error");
    }
  };

  const swap = () => {
    setSource(targetBranch);
    setTargetBranch(source);
  };

  const mono = "var(--font-mono, monospace)";
  const branchNames = branches?.branches.map((b) => b.name) ?? [];
  const branchOptions = branchNames.map((n) => ({ value: n, label: n }));

  return (
    <div
      role="dialog"
      aria-label={t("merge.title")}
      onMouseDown={() => {
        // Disable backdrop dismissal during a merge to avoid interruption.
        if (phase !== "working") close();
      }}
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
          width: 480,
          maxWidth: "calc(100vw - 32px)",
          maxHeight: "calc(100vh - 64px)",
          overflowY: "auto",
          background: "var(--bg-panel)",
          border: "1px solid var(--border-strong)",
          borderRadius: 10,
          padding: 18,
          boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
        }}
      >
        <div
          style={{
            fontSize: 14,
            fontWeight: 600,
            color: "var(--text-primary)",
            marginBottom: 6,
          }}
        >
          {t("merge.title")}
        </div>
        <div style={{ fontSize: 12, color: "var(--text-muted)", marginBottom: 14 }}>
          {t("merge.desc")}
        </div>

        {phase === "loading" && (
          <div style={{ fontSize: 13, color: "var(--text-muted)" }}>
            {t("merge.loadingBranches")}
          </div>
        )}

        {phase === "notRepo" && (
          <div style={{ fontSize: 13, color: "var(--text-muted)" }}>
            {t("merge.notRepo")}
          </div>
        )}

        {(phase === "ready" || phase === "working") && branches && (
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            {/* Direction: source branch to target branch, with a swap action. */}
            <div style={{ display: "flex", alignItems: "flex-end", gap: 8 }}>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>
                  {t("merge.sourceLabel")}
                </div>
                <Select
                  value={source}
                  options={branchOptions}
                  placeholder={t("merge.selectBranch")}
                  disabled={phase === "working"}
                  width="100%"
                  mono
                  onChange={setSource}
                />
              </div>
              <button
                type="button"
                className="vlx-btn"
                title={t("merge.swap")}
                aria-label={t("merge.swap")}
                onClick={swap}
                disabled={phase === "working" || (!source && !targetBranch)}
                style={{ height: 32, padding: "0 9px", fontSize: 14, flex: "none" }}
              >
                ⇄
              </button>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>
                  {t("merge.targetLabel")}
                </div>
                <Select
                  value={targetBranch}
                  options={branchOptions}
                  placeholder={t("merge.selectBranch")}
                  disabled={phase === "working"}
                  width="100%"
                  mono
                  onChange={setTargetBranch}
                />
              </div>
            </div>

            {/* Preflight: identical/incomplete selections, loading, diff summary, and notices. */}
            {sameBranch ? (
              <div style={{ fontSize: 11.5, color: "var(--warning, #c9a227)" }}>
                {t("merge.sameBranch")}
              </div>
            ) : !source || !targetBranch ? (
              <div style={{ fontSize: 11.5, color: "var(--text-muted)" }}>
                {t("merge.pickHint")}
              </div>
            ) : previewLoading ? (
              <div style={{ fontSize: 12.5, color: "var(--text-muted)" }}>
                {t("merge.loadingDiff")}
              </div>
            ) : preview ? (
              <>
                <div>
                  <div style={{ fontSize: 11, color: "var(--text-muted)", marginBottom: 4 }}>
                    {t("merge.changes", preview.target)}
                  </div>
                  <pre
                    style={{
                      margin: 0,
                      padding: 10,
                      maxHeight: 200,
                      overflow: "auto",
                      background: "var(--bg-app)",
                      border: "1px solid var(--border)",
                      borderRadius: 6,
                      fontFamily: mono,
                      fontSize: 12,
                      lineHeight: 1.5,
                      color: "var(--text-primary)",
                      whiteSpace: "pre-wrap",
                    }}
                  >
                    {preview.diffStat.trim() || t("merge.noChanges")}
                  </pre>
                </div>

                {/* Why merging is unavailable, such as an unchecked-out target or missing branch. */}
                {!preview.available && (
                  <div style={{ fontSize: 11.5, color: "var(--warning, #c9a227)" }}>
                    {reasonText(preview.reason)}
                  </div>
                )}
                {/* Already up to date: the source is contained in the target. */}
                {preview.available && preview.upToDate && (
                  <div style={{ fontSize: 11.5, color: "var(--text-muted)" }}>
                    {t("merge.upToDate")}
                  </div>
                )}
                {/* Warn, but do not block, when the target worktree is dirty. */}
                {preview.available && !preview.upToDate && preview.targetDirty && (
                  <div style={{ fontSize: 11.5, color: "var(--warning, #c9a227)" }}>
                    {t("merge.targetDirty")}
                  </div>
                )}
                {/* Commit a dirty source worktree first using the user's message. */}
                {preview.available && !preview.upToDate && preview.sourceDirty && (
                  <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                    <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
                      {t("merge.commitMsgLabel")}
                    </div>
                    <input
                      className="vlx-input"
                      type="text"
                      value={commitMsg}
                      onChange={(e) => setCommitMsg(e.target.value)}
                      placeholder={t("merge.commitMsgPlaceholder")}
                      disabled={phase === "working"}
                      style={{ boxSizing: "border-box" }}
                    />
                    <div style={{ fontSize: 11.5, color: "var(--text-muted)" }}>
                      {t("merge.sourceDirtyNote")}
                    </div>
                  </div>
                )}
              </>
            ) : null}
          </div>
        )}

        {phase === "conflict" && (
          <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
            <div style={{ fontSize: 12.5, color: "var(--text-primary)" }}>
              {t("merge.conflictMsg", targetBranch)}
            </div>
            <pre
              style={{
                margin: 0,
                padding: 10,
                background: "var(--bg-app)",
                border: "1px solid var(--border)",
                borderRadius: 6,
                fontFamily: mono,
                fontSize: 12,
                color: "var(--text-primary)",
                whiteSpace: "pre-wrap",
              }}
            >
              {conflicts.join("\n")}
            </pre>
          </div>
        )}

        {phase === "done" && (
          <div style={{ fontSize: 13, color: "var(--text-primary)" }}>
            {t("merge.doneMsg", source, targetBranch)}
          </div>
        )}

        {phase === "error" && (
          <div
            style={{
              fontSize: 12.5,
              color: "var(--danger, #e05252)",
              whiteSpace: "pre-wrap",
            }}
          >
            {error}
          </div>
        )}

        <div
          style={{
            display: "flex",
            justifyContent: "flex-end",
            gap: 8,
            marginTop: 18,
          }}
        >
          {(phase === "ready" || phase === "working") && (
            <>
              <button className="vlx-btn" onClick={close} disabled={phase === "working"}>
                {t("common.cancel")}
              </button>
              <button
                className="vlx-btn vlx-btn-primary"
                onClick={() => void doMerge()}
                disabled={phase === "working" || !canMerge}
              >
                {phase === "working"
                  ? t("merge.working")
                  : preview?.sourceDirty
                    ? t("merge.commitAndApply")
                    : t("merge.apply")}
              </button>
            </>
          )}
          {(phase === "done" ||
            phase === "conflict" ||
            phase === "error" ||
            phase === "loading" ||
            phase === "notRepo") && (
            <button className="vlx-btn vlx-btn-primary" onClick={close}>
              {t("merge.close")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
