//! One of a Claude conversation's background tasks, opened as a tab of its own.
//!
//! The view owns its data: it subscribes to the session's chat events and reads its task out of every
//! `extras` event, so it keeps working after the conversation pane unmounts (the process stays alive while
//! a task runs). The opener's copy of the task is the first paint; one snapshot on mount fetches the current
//! state and, in the browser, registers this connection for the session's events. A task that leaves the
//! list without a terminal frame is shown as ended with the last state seen, never as running or empty.

import { useEffect, useState } from "react";

import Icons from "../../../components/Icons";
import { fmtTokens } from "../../../format";
import { dateLocale, useT, type I18nKey } from "../../../i18n";
import {
  chatSnapshot,
  chatStopTask,
  extrasOf,
  isTaskFinished,
  onChatEvent,
  type ChatBackgroundTask,
  type ChatWorkflowAgent,
  type ChatWorkflowPhase,
} from "../../../ipc/chat";
import type { TaskTab } from "../../../store/termStore";

/** The icon a task kind is drawn with, here and on its tab. */
export function taskIcon(taskType: string) {
  if (taskType === "local_workflow") return Icons.layers;
  if (taskType === "local_agent") return Icons.bot;
  return Icons.terminal;
}

/** The label key for a task status; unknown values are shown as the agent wrote them. */
export function taskStatusKey(status: string | undefined): I18nKey | null {
  switch (status) {
    case undefined:
    case "running":
    case "pending":
    case "paused":
      return "chat.tasks.status.running";
    case "completed":
      return "chat.tasks.status.completed";
    case "failed":
    case "error":
      return "chat.tasks.status.failed";
    case "killed":
    case "stopped":
    case "canceled":
    case "cancelled":
      return "chat.tasks.status.canceled";
    case "ended":
      return "chat.tasks.status.ended";
    default:
      return null;
  }
}

/** "1:05" or "1:02:03": the duration format the phase list and the elapsed figure share. */
export function fmtElapsed(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const seconds = total % 60;
  const minutes = Math.floor(total / 60) % 60;
  const hours = Math.floor(total / 3600);
  const mm = hours ? String(minutes).padStart(2, "0") : String(minutes);
  return `${hours ? `${hours}:` : ""}${mm}:${String(seconds).padStart(2, "0")}`;
}

/**
 * How long the task has run: wall clock from the backend's start stamp while it runs, start to end once it
 * has ended, and the agent's own `duration_ms` when the stamps are missing.
 */
function elapsedOf(task: ChatBackgroundTask | undefined, finished: boolean, now: number): number | undefined {
  if (task?.started_at == null) return task?.usage?.duration_ms;
  if (task.ended_at != null) return task.ended_at - task.started_at;
  if (!finished) return now - task.started_at;
  return task.usage?.duration_ms;
}

/** Human-readable task type: "local_workflow" reads as "local workflow". */
const typeLabel = (taskType: string) => taskType.replace(/_/g, " ");

export function TaskView({ tab, hidden }: { tab: TaskTab; hidden: boolean }) {
  const t = useT();
  const [task, setTask] = useState<ChatBackgroundTask | undefined>(tab.seed);
  /** The last list did not carry this task, and no terminal frame explained why. */
  const [stale, setStale] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    let disposed = false;
    let sawEvent = false;
    let unlisten: (() => void) | undefined;
    const apply = (tasks: ChatBackgroundTask[] | undefined) => {
      const mine = tasks?.find((candidate) => candidate.task_id === tab.taskId);
      if (mine) {
        setTask(mine);
        setStale(false);
      } else {
        setStale(true);
      }
    };
    void onChatEvent(tab.sessionId, (event) => {
      if (event.type !== "extras") return;
      sawEvent = true;
      apply(event.extras.backgroundTasks);
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    // The snapshot is the current state at mount; an event that arrived first is newer than it.
    void chatSnapshot(tab.sessionId)
      .then((snapshot) => {
        if (!disposed && !sawEvent) apply(extrasOf(snapshot).backgroundTasks);
      })
      .catch(() => {
        /* The seed stays; the subscription still delivers every later change. */
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [tab.sessionId, tab.taskId]);

  const finished = !task || isTaskFinished(task) || stale;
  useEffect(() => {
    if (finished || hidden) return;
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [finished, hidden]);

  const status = task && isTaskFinished(task) ? task.status : stale ? "ended" : task?.status;
  const statusKey = taskStatusKey(status);
  const statusLabel = statusKey ? t(statusKey) : (status ?? "");
  const elapsedMs = elapsedOf(task, finished, now);
  const TaskIcon = taskIcon(task?.task_type || tab.taskType);
  const progress = task?.workflow_progress ?? [];
  const phases = progress
    .filter((entry): entry is ChatWorkflowPhase => entry.type === "workflow_phase")
    .sort((a, b) => a.index - b.index);
  const agents = progress.filter((entry): entry is ChatWorkflowAgent => entry.type === "workflow_agent");
  const unphased = agents.filter((agent) => !phases.some((phase) => phase.index === agent.phaseIndex));
  const isWorkflow = (task?.task_type || tab.taskType) === "local_workflow";
  const when = (ms: number) => new Date(ms).toLocaleString(dateLocale());

  return (
    <div className={"sv-task-view" + (finished ? " sv-task-finished" : "")} style={hidden ? { display: "none" } : undefined}>
      <div className="sv-task-head">
        <span className="sv-task-icon"><TaskIcon size={16} /></span>
        <div className="sv-task-heading">
          <div className="sv-task-title">{tab.title}</div>
          <div className="sv-task-sub">
            <span>{typeLabel(task?.task_type || tab.taskType)}</span>
            <span className={"sv-task-badge sv-task-badge-" + (statusKey ? statusKey.split(".").pop() : "other")}>{statusLabel}</span>
            {stale && !(task && isTaskFinished(task)) ? <span className="sv-task-stale">{t("chat.tasks.stale")}</span> : null}
          </div>
        </div>
        {!finished ? (
          <button
            className="sv-popover-action"
            onClick={() => void chatStopTask(tab.sessionId, tab.taskId).catch((err) => setError(String(err)))}
          >
            {t("chat.tasks.stop")}
          </button>
        ) : null}
      </div>
      {error ? <div className="sv-task-error" role="alert">{error}</div> : null}

      {task?.description ? <div className="sv-task-description">{task.description}</div> : null}

      <dl className="sv-task-stats">
        {elapsedMs != null ? <Stat label={t("chat.tasks.elapsed")} value={fmtElapsed(elapsedMs)} /> : null}
        {task?.usage?.total_tokens != null ? <Stat label={t("chat.tasks.tokens")} value={fmtTokens(task.usage.total_tokens)} /> : null}
        {task?.usage?.tool_uses != null ? <Stat label={t("chat.tasks.toolUses")} value={String(task.usage.tool_uses)} /> : null}
        {task?.last_tool_name && !finished ? <Stat label={t("chat.tasks.currentAgent")} value={task.last_tool_name} /> : null}
        {task?.started_at != null ? <Stat label={t("chat.tasks.started")} value={when(task.started_at)} /> : null}
        {task?.ended_at != null ? <Stat label={t("chat.tasks.finished")} value={when(task.ended_at)} /> : null}
      </dl>

      {finished && task?.summary ? (
        <section className="sv-task-section">
          <h3>{t("chat.tasks.summary")}</h3>
          <div className="sv-task-preview">{task.summary}</div>
        </section>
      ) : null}
      {finished && task?.output_file ? (
        <section className="sv-task-section">
          <h3>{t("chat.tasks.outputFile")}</h3>
          <div className="sv-task-preview">{task.output_file}</div>
        </section>
      ) : null}

      {isWorkflow ? (
        <section className="sv-task-section">
          <h3>{t("chat.tasks.phases")}</h3>
          {agents.length === 0 && phases.length === 0 ? (
            <div className="sv-task-empty">{t("chat.tasks.noProgress")}</div>
          ) : (
            <>
              {phases.map((phase) => (
                <div className="sv-task-phase" key={phase.index}>
                  <div className="sv-task-phase-title">{phase.title}</div>
                  {agents
                    .filter((agent) => agent.phaseIndex === phase.index)
                    .map((agent) => <AgentRow key={agent.index} agent={agent} now={now} />)}
                </div>
              ))}
              {unphased.length ? (
                <div className="sv-task-phase">
                  {unphased.map((agent) => <AgentRow key={agent.index} agent={agent} now={now} />)}
                </div>
              ) : null}
            </>
          )}
        </section>
      ) : null}
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="sv-task-stat">
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

/** One agent of a workflow: who, on what model, in which state, and what it was asked and answered. */
function AgentRow({ agent, now }: { agent: ChatWorkflowAgent; now: number }) {
  const t = useT();
  const done = agent.state === "done";
  const durationMs =
    agent.durationMs ?? (agent.state === "start" && agent.startedAt != null ? now - agent.startedAt : undefined);
  const stateLabel =
    agent.state === "start" ? t("chat.tasks.agentState.start") : done ? t("chat.tasks.agentState.done") : (agent.state ?? "");
  return (
    <div className={"sv-task-agent" + (done ? " sv-task-agent-done" : agent.state === "start" ? " sv-task-agent-start" : "")}>
      <div className="sv-task-agent-head">
        <span className="sv-task-agent-label">{agent.label}</span>
        {agent.model ? <span className="sv-task-agent-meta">{agent.model}</span> : null}
        {stateLabel ? <span className="sv-task-agent-state">{stateLabel}</span> : null}
        {agent.attempt != null && agent.attempt > 1 ? <span className="sv-task-agent-meta">{t("chat.tasks.attempt", agent.attempt)}</span> : null}
        <span className="sv-task-agent-spacer" />
        {durationMs != null ? <span className="sv-task-agent-meta">{fmtElapsed(durationMs)}</span> : null}
        {agent.tokens != null ? <span className="sv-task-agent-meta">{t("chat.subagent.tokens", fmtTokens(agent.tokens))}</span> : null}
        {agent.toolCalls != null ? <span className="sv-task-agent-meta">{t("chat.subagent.steps", agent.toolCalls)}</span> : null}
      </div>
      {agent.promptPreview ? (
        <div className="sv-task-agent-text">
          <span className="sv-task-agent-key">{t("chat.tasks.prompt")}</span>
          <span className="sv-task-preview">{agent.promptPreview}</span>
        </div>
      ) : null}
      {done && agent.resultPreview ? (
        <div className="sv-task-agent-text">
          <span className="sv-task-agent-key">{t("chat.tasks.result")}</span>
          <span className="sv-task-preview">{agent.resultPreview}</span>
        </div>
      ) : null}
    </div>
  );
}
