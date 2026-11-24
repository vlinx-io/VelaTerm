//! Summary bar rendered at the top of `ChangesView` and `DiffDocView` whenever inline diff review
//! comments have been drafted for the current comparison. Lets the user inspect or remove individual
//! comments, pick which repository agent session receives them, copy the formatted review, or send
//! it straight to the agent.

import { useEffect, useMemo, useState } from "react";
import Icons from "../Icons";
import Select from "../Select";
import { useT } from "../../i18n";
import { copyText } from "../../ipc/info";
import { useTermStore } from "../../store/termStore";
import type { Project, Session, SessionRuntime } from "../../types";
import {
  deliverReviewToCandidate,
  formatLineRange,
  formatReviewPrompt,
  resolveReviewCandidates,
  useDiffReviewStore,
  useScopeComments,
} from "./diffReviewStore";

const EMPTY_SESSIONS: Session[] = [];
const EMPTY_PROJECTS: Project[] = [];
const EMPTY_RUNTIMES: Record<string, SessionRuntime | undefined> = {};

export function DiffReviewBar({
  scopeKey,
  cwd,
  onSelectFile,
}: {
  scopeKey: string;
  cwd: string;
  onSelectFile?: (path: string) => void;
}) {
  const t = useT();
  const comments = useScopeComments(scopeKey);
  const removeComment = useDiffReviewStore((s) => s.removeComment);
  const clearScope = useDiffReviewStore((s) => s.clearScope);
  const lastActiveSessionId = useDiffReviewStore((s) => s.lastActiveSessionId);

  const sessions = useTermStore((s) => (s as Partial<typeof s>)?.sessions ?? EMPTY_SESSIONS);
  const projects = useTermStore((s) => (s as Partial<typeof s>)?.projects ?? EMPTY_PROJECTS);
  const runtimes = useTermStore((s) => (s as Partial<typeof s>)?.runtimes ?? EMPTY_RUNTIMES);
  const activeSessionId = useTermStore((s) => (s as Partial<typeof s>)?.activeSessionId ?? null);

  const [listOpen, setListOpen] = useState(false);
  const [pickedSessionId, setPickedSessionId] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const [copied, setCopied] = useState(false);
  const [sendErr, setSendErr] = useState("");

  const candidates = useMemo(
    () =>
      resolveReviewCandidates(
        cwd,
        sessions,
        projects,
        runtimes,
        activeSessionId ?? lastActiveSessionId,
      ),
    [cwd, sessions, projects, runtimes, activeSessionId, lastActiveSessionId],
  );

  const chosen = useMemo(() => {
    if (pickedSessionId) {
      const found = candidates.find((c) => c.session.id === pickedSessionId);
      if (found) return found;
    }
    return candidates[0] ?? null;
  }, [candidates, pickedSessionId]);

  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), 1800);
    return () => clearTimeout(timer);
  }, [copied]);

  if (comments.length === 0) return null;

  const fileCount = new Set(comments.map((c) => c.path)).size;

  const handleCopy = async () => {
    const prompt = formatReviewPrompt(comments, t);
    await copyText(prompt);
    setCopied(true);
  };

  const handleSend = async () => {
    if (!chosen || sending) return;
    setSending(true);
    setSendErr("");
    try {
      const prompt = formatReviewPrompt(comments, t);
      await deliverReviewToCandidate(chosen, prompt);
      clearScope(scopeKey);
      setListOpen(false);
    } catch (e) {
      setSendErr(String(e));
    } finally {
      setSending(false);
    }
  };

  return (
    <div className="diff-review-bar">
      <div className="diff-review-bar-main">
        <span className="diff-review-summary">
          <Icons.chat size={13} aria-hidden="true" />
          <span>{t("changes.review.count", comments.length, fileCount)}</span>
        </span>
        <button
          type="button"
          className="vlx-btn"
          aria-expanded={listOpen}
          onClick={() => setListOpen((v) => !v)}
        >
          {listOpen ? t("changes.review.hideList") : t("changes.review.showList")}
        </button>
        <div className="diff-review-spacer" />
        {candidates.length > 1 && chosen && (
          <Select
            value={chosen.session.id}
            onChange={setPickedSessionId}
            options={candidates.map((c) => ({ value: c.session.id, label: c.label }))}
            size="sm"
            width={210}
            ariaLabel={t("changes.review.targetAgent")}
            menuPortal
          />
        )}
        {candidates.length === 0 && (
          <span className="diff-review-hint">{t("changes.review.noAgent")}</span>
        )}
        <button type="button" className="vlx-btn" onClick={() => void handleCopy()}>
          <Icons.copy size={12} aria-hidden="true" />
          <span>{copied ? t("changes.review.copied") : t("changes.review.copy")}</span>
        </button>
        <button
          type="button"
          className="vlx-btn"
          disabled={sending}
          onClick={() => {
            clearScope(scopeKey);
            setListOpen(false);
          }}
        >
          {t("changes.review.clearAll")}
        </button>
        {chosen && (
          <button
            type="button"
            className="vlx-btn vlx-btn-primary"
            disabled={sending}
            onClick={() => void handleSend()}
          >
            <Icons.sparkle size={12} aria-hidden="true" />
            <span>
              {sending
                ? t("changes.review.sending")
                : chosen.sends
                  ? t("changes.review.sendTo", chosen.agentLabel)
                  : t("changes.review.pasteTo", chosen.agentLabel)}
            </span>
          </button>
        )}
      </div>
      {sendErr && (
        <div className="diff-review-error" role="alert">
          {sendErr}
        </div>
      )}
      {listOpen && (
        <ul className="diff-review-list" role="list">
          {comments.map((c) => (
            <li key={c.id} className="diff-review-list-item">
              <button
                type="button"
                className="diff-review-list-jump"
                onClick={() => onSelectFile?.(c.path)}
              >
                <span className="diff-review-list-loc">
                  {c.path} · {c.side === "original" ? t("changes.review.sideOriginal") : t("changes.review.sideModified")}{" "}
                  {formatLineRange(c.startLine, c.endLine)}
                </span>
                <span className="diff-review-list-body">{c.body}</span>
              </button>
              <button
                type="button"
                className="vlx-btn diff-review-item-remove"
                aria-label={`${t("changes.review.remove")} ${c.path} ${formatLineRange(c.startLine, c.endLine)}`}
                onClick={() => removeComment(scopeKey, c.id)}
              >
                {t("changes.review.remove")}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
