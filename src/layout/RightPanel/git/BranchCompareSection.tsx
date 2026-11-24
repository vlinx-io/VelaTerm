//! "Committed on this branch": the files the current branch changed since it split from a base
//! branch, with how far each side has moved, paired with a full pull/merge request loop across
//! GitHub (`gh`), GitLab (`glab`), and configured Gitea servers.
//!
//! When an open, draft, merged, or closed PR/MR exists for the branch, its status and CI check
//! summary appear directly in the section, and a merged worktree offers one-click cleanup.

import { useCallback, useEffect, useState } from "react";
import { FormModal, type FieldDef } from "../../../components/FormModal";
import Icons from "../../../components/Icons";
import Select from "../../../components/Select";
import { invalidateGitBranch } from "../../../hooks/useGitBranch";
import { useT } from "../../../i18n";
import {
  deleteBranch,
  gitBranchCompare,
  gitOpenPullRequest,
  gitPrProvider,
  gitPrStatus,
  removeWorktree,
  type BranchCompare,
  type GitPrProvider,
  type GitPrStatus,
  type PrOutcome,
} from "../../../ipc/commands";
import { platform } from "../../../platform";
import { useTermStore } from "../../../store/termStore";
import type { Session } from "../../../types";
import { normalizeRepoPath } from "../../../components/diff/diffReviewStore";
import { GitFileRow } from "./GitFileRow";
import { GitSection } from "./GitSection";

const BASE_KEY = "vlx.git.compareBase:";

function readBase(root: string): string | null {
  try {
    return localStorage.getItem(BASE_KEY + root);
  } catch {
    return null;
  }
}

function writeBase(root: string, base: string): void {
  try {
    localStorage.setItem(BASE_KEY + root, base);
  } catch {
    // The choice then lasts until the tab closes.
  }
}

export function BranchCompareSection({
  cwd,
  root,
  head,
  branch,
  refreshToken,
  open,
  onToggle,
  session,
}: {
  cwd: string;
  /** Checked-out branch, the head of a pull request; null when detached. */
  branch: string | null;
  /** Repository root, which keys the remembered base. */
  root: string;
  /** HEAD identity; a new commit reloads the comparison. */
  head: string | null;
  refreshToken: number;
  open: boolean;
  onToggle: () => void;
  session?: Session;
}) {
  const t = useT();
  const openChanges = useTermStore((s) => s.openChanges);
  const archiveSession = useTermStore((s) => (s as Partial<typeof s>)?.archiveSession);
  const clearNodeWorktree = useTermStore((s) => (s as Partial<typeof s>)?.clearNodeWorktree);

  const [base, setBase] = useState<string | null>(() => readBase(root));
  const [data, setData] = useState<BranchCompare | null>(null);
  const [err, setErr] = useState("");
  const [provider, setProvider] = useState<GitPrProvider | null>(null);
  const [prStatus, setPrStatus] = useState<GitPrStatus | null>(null);
  const [statusTick, setStatusTick] = useState(0);
  const [prForm, setPrForm] = useState(false);
  const [cleanupForm, setCleanupForm] = useState(false);
  const [opened, setOpened] = useState<PrOutcome | null>(null);

  useEffect(() => setBase(readBase(root)), [root]);

  // Asked once per repository: it may probe Gitea over the network or check gh/glab auth status.
  useEffect(() => {
    let alive = true;
    setProvider(null);
    setPrStatus(null);
    setOpened(null);
    void gitPrProvider(cwd)
      .then((p) => {
        if (alive) setProvider(p);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [cwd]);

  const reloadPrStatus = useCallback(() => {
    setStatusTick((n) => n + 1);
  }, []);

  useEffect(() => {
    if (!provider?.available || !branch || typeof gitPrStatus !== "function") {
      setPrStatus(null);
      return;
    }
    let alive = true;
    void gitPrStatus(cwd)
      .then((s) => {
        if (alive) setPrStatus(s);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [cwd, provider, branch, head, refreshToken, statusTick]);

  useEffect(() => {
    if (!head) {
      setData(null);
      return;
    }
    let alive = true;
    void gitBranchCompare(cwd, base)
      .then((result) => {
        if (!alive) return;
        setData(result);
        setErr("");
      })
      .catch((e) => {
        if (!alive) return;
        // A remembered base that no longer exists falls back to the default.
        if (base && /not found/i.test(String(e))) {
          setBase(null);
          return;
        }
        setErr(String(e));
      });
    return () => {
      alive = false;
    };
  }, [cwd, base, head, refreshToken]);

  if (!data && !err) return null;

  const openFile = (path?: string) => {
    if (!data) return;
    openChanges(cwd, { path, commit: data.head, base: { from: data.mergeBase, label: data.base } });
  };
  const choose = (next: string) => {
    writeBase(root, next);
    setBase(next);
  };

  const isGitLab = provider?.provider === "gitlab";
  const openLabel = isGitLab ? t("git.pr.openMr") : t("git.pr.open");
  const openedLabel = isGitLab ? t("git.pr.openedMr") : t("git.pr.opened");

  const unavailableReason =
    provider && !provider.available
      ? provider.reason === "no_token"
        ? t("git.pr.noToken")
        : provider.reason === "unreachable"
          ? t("git.pr.unreachable")
          : provider.reason === "no_cli"
            ? isGitLab
              ? t("git.pr.noGlabCli")
              : t("git.pr.noGhCli")
            : provider.reason === "no_auth"
              ? isGitLab
                ? t("git.pr.noGlabAuth")
                : t("git.pr.noGhAuth")
              : undefined
      : undefined;

  const hasActiveOrMergedPr =
    prStatus != null && (prStatus.state === "open" || prStatus.state === "draft" || prStatus.state === "merged");
  const canPromptCreate = Boolean(provider?.provider && branch && data && data.ahead > 0 && !hasActiveOrMergedPr);

  const prPrefix = prStatus?.provider === "gitlab" ? "!" : "#";
  const sessionMatchesWorktree = Boolean(
    session &&
      prStatus?.worktree &&
      (normalizeRepoPath(session.worktreePath ?? "") === normalizeRepoPath(prStatus.worktree.path) ||
        normalizeRepoPath(session.cwd ?? "") === normalizeRepoPath(prStatus.worktree.path)),
  );

  const cleanupFields: FieldDef[] = prStatus?.worktree && branch
    ? [
        {
          key: "deleteBranch",
          type: "checkbox",
          label: t("git.pr.cleanupDeleteBranch", branch),
        },
        ...(sessionMatchesWorktree
          ? [
              {
                key: "archiveSession",
                type: "checkbox" as const,
                label: t("git.pr.cleanupArchiveSession"),
                hint: t("git.pr.cleanupArchiveHint"),
              },
            ]
          : []),
        {
          key: "force",
          type: "checkbox",
          label: t("git.pr.cleanupForce"),
        },
      ]
    : [];

  return (
    <GitSection
      title={t("git.branchChanges")}
      count={data?.files.length ?? 0}
      open={open}
      onToggle={onToggle}
      view={data && data.files.length > 0 ? { label: t("git.viewAll"), onClick: () => openFile() } : undefined}
    >
      {err && <div className="insp-empty git-err">{err}</div>}
      {data && (
        <>
          <div className="git-compare-head">
            <Select
              value={data.base}
              onChange={choose}
              options={[...new Set([data.base, ...data.candidates])].map((name) => ({ value: name, label: name }))}
              width="100%"
              size="sm"
              leading={t("git.compareBase")}
              title={data.base}
              menuPortal
            />
            <span className="git-compare-counts" title={t("git.compareAheadBehind", data.ahead, data.behind, data.base)}>
              ↑{data.ahead} ↓{data.behind}
            </span>
          </div>

          {prStatus && (
            <div className="git-pr-card">
              <div className="git-pr-card-top">
                <span className={`git-pr-badge is-${prStatus.state}`}>
                  {t(`git.pr.state.${prStatus.state}`)}
                </span>
                <a
                  href={prStatus.url}
                  className="git-pr-link"
                  onClick={(e) => {
                    e.preventDefault();
                    if (prStatus.url) void platform.opener.openExternal(prStatus.url);
                  }}
                >
                  {prStatus.number > 0 ? `${prPrefix}${prStatus.number} ` : ""}
                  {prStatus.title || openedLabel}
                </a>
                <button
                  type="button"
                  className="icon-btn sm"
                  aria-label={t("git.pr.refreshStatus")}
                  title={t("git.pr.refreshStatus")}
                  onClick={reloadPrStatus}
                >
                  <Icons.restart size={12} aria-hidden="true" />
                </button>
              </div>
              {(prStatus.checks !== "none" || (prStatus.state === "merged" && prStatus.worktree)) && (
                <div className="git-pr-card-meta">
                  {prStatus.checks !== "none" && (
                    <span className={`git-pr-checks is-${prStatus.checks}`}>
                      {prStatus.checks === "passing" ? (
                        <Icons.check size={11} aria-hidden="true" />
                      ) : prStatus.checks === "failing" ? (
                        <Icons.x size={11} aria-hidden="true" />
                      ) : (
                        <Icons.clock size={11} aria-hidden="true" />
                      )}
                      <span>{t(`git.pr.checks.${prStatus.checks}`)}</span>
                    </span>
                  )}
                  {prStatus.state === "merged" && prStatus.worktree && (
                    <button
                      type="button"
                      className="vlx-btn git-pr-cleanup-btn"
                      onClick={() => setCleanupForm(true)}
                    >
                      {t("git.pr.cleanupWorktree")}
                    </button>
                  )}
                </div>
              )}
            </div>
          )}

          {canPromptCreate && provider && (
            <div className="git-pr">
              <button
                type="button"
                className="vlx-btn"
                disabled={!provider.available}
                title={unavailableReason}
                onClick={() => { setOpened(null); setPrForm(true); }}
              >
                {openLabel}
              </button>
              {!provider.available && unavailableReason && (
                <span className="git-pr-note">{unavailableReason}</span>
              )}
              {opened && !prStatus && (
                <span className="git-pr-note">
                  {opened.url
                    ? <a href={opened.url} onClick={(e) => { e.preventDefault(); void platform.opener.openExternal(opened.url); }}>{openedLabel}</a>
                    : openedLabel}
                </span>
              )}
            </div>
          )}

          {data.files.length === 0 && <div className="insp-empty">{t("git.noBranchChanges")}</div>}
          {data.files.map((f) => (
            <GitFileRow
              key={f.path}
              file={f}
              additions={f.additions}
              deletions={f.deletions}
              binary={f.binary}
              onOpen={() => openFile(f.path)}
              actions={[]}
            />
          ))}
        </>
      )}

      {prForm && data && branch && (
        <FormModal
          title={
            provider?.provider === "gitea"
              ? t("git.pr.formTitle")
              : t("git.pr.formTitleProvider", provider?.provider ?? "github")
          }
          description={
            isGitLab
              ? t("git.pr.formDescriptionMr", branch, data.base)
              : t("git.pr.formDescription", branch, data.base)
          }
          fields={[
            { key: "title", label: t("git.pr.titleField"), autoFocus: true, placeholder: branch },
            {
              key: "body",
              label: t("git.pr.bodyField"),
              render: (value, onChange, context) => (
                <textarea className="vlx-input git-pr-body" rows={5} value={value} disabled={context.disabled}
                  onChange={(e) => onChange(e.target.value)} />
              ),
            },
            {
              key: "draft",
              type: "checkbox",
              label: t("git.pr.draftField"),
            },
          ]}
          initial={{
            title: data.suggestedTitle ?? "",
            body: data.suggestedBody ?? "",
            draft: "",
          }}
          submitLabel={openLabel}
          submittingLabel={t("git.pr.opening")}
          width={460}
          onSubmit={async (values) => {
            const draft = values.draft === "1";
            const outcome = draft
              ? await gitOpenPullRequest(cwd, data.base, values.title ?? "", values.body ?? "", true)
              : await gitOpenPullRequest(cwd, data.base, values.title ?? "", values.body ?? "");
            setOpened(outcome);
            setPrForm(false);
            reloadPrStatus();
          }}
          onCancel={() => setPrForm(false)}
        />
      )}

      {cleanupForm && prStatus?.worktree && branch && (
        <FormModal
          title={t("git.pr.cleanupTitle")}
          description={t("git.pr.cleanupDescription", branch, prStatus.worktree.path)}
          fields={cleanupFields}
          initial={{
            deleteBranch: "1",
            ...(sessionMatchesWorktree ? { archiveSession: "1" } : {}),
            force: "",
          }}
          submitLabel={t("git.pr.cleanupSubmit")}
          submittingLabel={t("git.pr.cleanupSubmitting")}
          width={440}
          onSubmit={async (values) => {
            const wt = prStatus.worktree!;
            const force = values.force === "1";
            await removeWorktree(wt.path, force);
            if (values.deleteBranch === "1") {
              await deleteBranch(wt.mainRepo, branch).catch(() => {});
            }
            if (sessionMatchesWorktree && session) {
              if (values.archiveSession === "1") {
                await archiveSession?.(session.id);
              } else {
                await clearNodeWorktree?.("session", session.id);
              }
            }
            invalidateGitBranch(wt.path);
            setCleanupForm(false);
          }}
          onCancel={() => setCleanupForm(false)}
        />
      )}
    </GitSection>
  );
}
