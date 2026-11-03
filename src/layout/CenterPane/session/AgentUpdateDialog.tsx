//! Confirms and runs an agent's self-update from the version badge.
//!
//! The dialog shows the exact command first, the same way the install card does, and runs it only after the
//! user confirms. The backend owns the process, so closing the dialog does not stop the update; opening it
//! again picks the running update back up. A conversation keeps the version it started with, so a finished
//! update offers to restart the agent, which keeps the conversation and starts the new version on the next
//! message.

import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Backdrop } from "../../../components/Backdrop";
import { useT } from "../../../i18n";
import { agentUpdateStart, agentUpdateStatus, type AgentUpdateStatus } from "../../../ipc/agentVersion";
import { chatDetach } from "../../../ipc/chat";
import { copyText } from "../../../ipc/info";
import { platform } from "../../../platform";
import type { Session } from "../../../types";
import { AGENT_KIND_LABEL, kindIconEl } from "../../sessionViewers/sessionMeta";
import { parseAnsi, spanCss } from "../ansiLog";

/** How often a running update is read again. */
const POLL_MS = 1000;

export function AgentUpdateDialog({
  session,
  from,
  to,
  command,
  docsUrl,
  agentRunning,
  onClose,
  onFinished,
}: {
  session: Session;
  from: string;
  to: string;
  command: string;
  docsUrl: string | null;
  /** Whether this conversation's agent process is live and therefore still on the old version. */
  agentRunning: boolean;
  onClose: () => void;
  /** Read the version again once the command has finished. */
  onFinished: () => void;
}) {
  const t = useT();
  const label = AGENT_KIND_LABEL[session.kind] ?? session.kind;
  // Null until the backend says whether an update for this executable is already running.
  const [status, setStatus] = useState<AgentUpdateStatus | null>(null);
  const [started, setStarted] = useState(false);
  const [startFailed, setStartFailed] = useState(false);
  const [copied, setCopied] = useState(false);
  const [restarted, setRestarted] = useState(false);
  const log = useRef<HTMLPreElement>(null);
  const finishedRef = useRef(onFinished);
  finishedRef.current = onFinished;

  const running = status?.state === "running";
  // A result from an earlier run belongs to that run; the dialog shows only one it started or watched.
  const showsRun = running || (started && !!status && status.state !== "idle");

  useEffect(() => {
    let alive = true;
    void agentUpdateStatus(session.id)
      .then((next) => {
        if (!alive) return;
        setStatus(next);
        if (next.state === "running") setStarted(true);
      })
      .catch(() => {
        if (alive) setStatus({ state: "idle", output: "", exitCode: null, timedOut: false, version: null });
      });
    return () => {
      alive = false;
    };
  }, [session.id]);

  useEffect(() => {
    if (!running) return;
    let alive = true;
    const timer = window.setInterval(() => {
      void agentUpdateStatus(session.id)
        .then((next) => {
          // A running update never goes back to idle; such an answer is a lookup miss, so keep polling.
          if (!alive || next.state === "idle") return;
          setStatus(next);
          if (next.state !== "running") finishedRef.current();
        })
        .catch(() => {});
    }, POLL_MS);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [running, session.id]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  // Follow the end of the output as it grows.
  useLayoutEffect(() => {
    const el = log.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [status?.output]);

  const spans = useMemo(() => (status?.output ? parseAnsi(status.output) : []), [status?.output]);

  const start = () => {
    setStarted(true);
    setStartFailed(false);
    void agentUpdateStart(session.id)
      .then((next) => {
        setStatus(next);
        if (next.state !== "running") onFinished();
      })
      .catch(() => {
        setStarted(false);
        setStartFailed(true);
      });
  };
  const copy = () => {
    void copyText(command);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1400);
  };
  const openDocs = () => {
    if (docsUrl) void platform.opener.openExternal(docsUrl).catch(() => {});
  };
  // The process ends once it is idle; an answer still being written finishes first.
  const restart = () => {
    setRestarted(true);
    void chatDetach(session.id).catch(() => {}).finally(onClose);
  };

  const succeeded = status?.state === "succeeded";
  const updated = succeeded && status?.version != null && status.version !== from;
  let outcome: string | null = null;
  if (status && showsRun && !running) {
    if (status.state === "failed") outcome = t(status.timedOut ? "agentUpdate.timedOut" : "agentUpdate.failed");
    else if (updated) outcome = t("agentUpdate.succeeded", label, status.version!);
    else if (status.version) outcome = t("agentUpdate.unchanged", status.version);
    else outcome = t("agentUpdate.finished");
  }

  // Portaled: the conversation pane uses layout containment, which would confine a fixed overlay to the pane.
  return createPortal(
    <Backdrop onClose={onClose}>
      <div
        className="agent-update"
        role="dialog"
        aria-modal="true"
        aria-label={t("agentUpdate.title", label)}
        onClick={(event) => event.stopPropagation()}
      >
        <header className="agent-update-head">
          <span className="agent-update-icon">{kindIconEl(session.kind, 16)}</span>
          <span className="agent-update-title">{t("agentUpdate.title", label)}</span>
          <span className="agent-update-versions">{`v${from} → v${to}`}</span>
        </header>

        <div className="agent-update-body">
          <div className="agent-update-hint">{t("agentUpdate.commandHint")}</div>
          <div className="agent-update-command">
            <code>{command}</code>
            <button type="button" className="vlx-btn" onClick={copy}>
              {copied ? t("common.copied") : t("common.copy")}
            </button>
          </div>

          {showsRun && (
            <>
              <pre ref={log} className="agent-update-log">
                {spans.length
                  ? spans.map((span, i) => {
                      const css = spanCss(span.style);
                      return Object.keys(css).length ? (
                        <span key={i} style={css}>
                          {span.text}
                        </span>
                      ) : (
                        span.text
                      );
                    })
                  : t("agentUpdate.waiting")}
              </pre>
              <div
                className={"agent-update-status" + (status?.state === "failed" ? " is-failed" : updated ? " is-done" : "")}
                role="status"
              >
                {running ? `${t("agentUpdate.running")} ${t("agentUpdate.background")}` : outcome}
              </div>
              {updated && agentRunning && !restarted && (
                <div className="agent-update-hint">{t("agentUpdate.restartHint")}</div>
              )}
            </>
          )}
          {startFailed && <div className="agent-update-status is-failed">{t("agentUpdate.startFailed")}</div>}
        </div>

        <footer className="agent-update-foot">
          {docsUrl && (
            <button type="button" className="agent-update-link" onClick={openDocs}>
              {t("agentUpdate.docs")}
            </button>
          )}
          <span className="agent-update-spacer" />
          {!showsRun ? (
            <>
              <button type="button" className="vlx-btn" onClick={onClose}>
                {t("common.cancel")}
              </button>
              <button type="button" className="vlx-btn vlx-btn-primary" disabled={!status || started} onClick={start}>
                {t("agentUpdate.confirm")}
              </button>
            </>
          ) : updated && agentRunning ? (
            <>
              <button type="button" className="vlx-btn" onClick={onClose}>
                {t("agentInstall.later")}
              </button>
              <button type="button" className="vlx-btn vlx-btn-primary" disabled={restarted} onClick={restart}>
                {t("agentUpdate.restart")}
              </button>
            </>
          ) : (
            <button type="button" className={"vlx-btn" + (running ? "" : " vlx-btn-primary")} onClick={onClose}>
              {t("common.close")}
            </button>
          )}
        </footer>
      </div>
    </Backdrop>,
    document.body,
  );
}
