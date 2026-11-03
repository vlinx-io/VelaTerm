//! The row above a terminal that lists the session's `vrun` commands while they run.
//!
//! Once a terminal agent sends a command to the background, its own screen no longer shows it, and
//! VelaTerm cannot draw inside that screen. This row is where a terminal session shows what is still
//! running: each command's label, how long it has run, its log, and a way to stop it. It takes no space
//! while nothing runs.

import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { Backdrop } from "../../components/Backdrop";
import { StatusIndicator } from "../../components/StatusIndicator";
import { useT } from "../../i18n";
import { runLogTail, runStop, type BackgroundRun, type RunLog } from "../../ipc/runs";
import { useTermStore } from "../../store/termStore";
import type { SessionId } from "../../types";
import { fmtElapsed } from "./session/taskMeta";
import { parseAnsi, spanCss } from "./ansiLog";
import "./run-strip.css";

/** How long the stop button waits for its confirming second click. */
const CONFIRM_WINDOW_MS = 4000;
/** How often an open log of a running command is read again. */
const LOG_REFRESH_MS = 2000;

export function RunStrip({ sessionId }: { sessionId: SessionId }) {
  const t = useT();
  const runs = useTermStore((s) => s.backgroundRuns[sessionId]);
  const [now, setNow] = useState(() => Date.now());
  const [viewing, setViewing] = useState<string | null>(null);
  const running = (runs?.length ?? 0) > 0;

  useEffect(() => {
    if (!running) return;
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [running]);

  return (
    <>
      {running && (
        <div className="run-strip" role="status" aria-label={t("term.runs.label")}>
          {runs!.map((run) => (
            <RunItem key={run.label} run={run} now={now} onViewLog={() => setViewing(run.label)} />
          ))}
        </div>
      )}
      {/* The log stays open after its command ends, so the outcome can still be read. */}
      {viewing && <RunLogDialog label={viewing} onClose={() => setViewing(null)} />}
    </>
  );
}

function RunItem({ run, now, onViewLog }: { run: BackgroundRun; now: number; onViewLog: () => void }) {
  const t = useT();
  const [confirming, setConfirming] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    if (!confirming) return;
    const timer = setTimeout(() => setConfirming(false), CONFIRM_WINDOW_MS);
    return () => clearTimeout(timer);
  }, [confirming]);

  const stop = () => {
    if (!confirming) {
      setConfirming(true);
      return;
    }
    setConfirming(false);
    setFailure(null);
    void runStop(run.label).catch((error) => setFailure(String(error)));
  };

  return (
    <div className="run-strip-item">
      <StatusIndicator status="working" />
      <span className="run-strip-label">{run.label}</span>
      <span className="run-strip-elapsed">{t("term.runs.elapsed", fmtElapsed(now - run.startedAt))}</span>
      <span className="run-strip-command" title={run.command}>{run.command}</span>
      {failure && <span className="run-strip-failure" title={failure}>{t("term.runs.stopFailed")}</span>}
      <button type="button" className="run-strip-action" onClick={onViewLog}>
        {t("term.runs.viewLog")}
      </button>
      <button type="button" className={`run-strip-action${confirming ? " is-confirming" : ""}`} onClick={stop}>
        {confirming ? t("term.runs.confirmStop") : t("term.runs.stop")}
      </button>
    </div>
  );
}

function RunLogDialog({ label, onClose }: { label: string; onClose: () => void }) {
  const t = useT();
  const [log, setLog] = useState<RunLog | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const text = useRef<HTMLPreElement>(null);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const load = () => {
      runLogTail(label)
        .then((next) => {
          if (cancelled) return;
          setLog(next);
          setFailure(null);
          if (next.running) timer = setTimeout(load, LOG_REFRESH_MS);
        })
        .catch((error) => {
          if (!cancelled) setFailure(String(error));
        });
    };
    load();
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, [label]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  // Follow the end of the log as it grows.
  useLayoutEffect(() => {
    const el = text.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [log?.text]);

  const spans = useMemo(() => (log?.text ? parseAnsi(log.text) : []), [log?.text]);

  const status = !log
    ? ""
    : log.running
      ? t("term.runs.logRunning")
      : log.exitCode != null
        ? t("term.runs.logFinished", log.exitCode)
        : t("term.runs.logEnded");

  return (
    <Backdrop onClose={onClose}>
      <div
        className="run-log"
        role="dialog"
        aria-modal="true"
        aria-label={t("term.runs.logTitle", label)}
        onClick={(event) => event.stopPropagation()}
      >
        <header className="run-log-header">
          <span className="run-log-title">{t("term.runs.logTitle", label)}</span>
          <span className="run-log-status">{status}</span>
          <button type="button" className="vlx-btn" onClick={onClose}>
            {t("common.close")}
          </button>
        </header>
        {failure ? (
          <div className="run-log-failure">{failure}</div>
        ) : (
          <pre ref={text} className="run-log-text">
            {log && !log.text
              ? t("term.runs.logEmpty")
              : spans.map((span, i) => {
                  const css = spanCss(span.style);
                  return Object.keys(css).length ? (
                    <span key={i} style={css}>
                      {span.text}
                    </span>
                  ) : (
                    span.text
                  );
                })}
          </pre>
        )}
      </div>
    </Backdrop>
  );
}
