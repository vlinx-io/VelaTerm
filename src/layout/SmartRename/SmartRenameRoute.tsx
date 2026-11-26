import { useEffect, useId, useRef, useState } from "react";
import AgentSelect from "../../components/AgentSelect";
import { Backdrop } from "../../components/Backdrop";
import { Field, FieldStack } from "../../components/Field";
import { FormModal } from "../../components/FormModal";
import Icons from "../../components/Icons";
import { LaunchLoadState, ModelEffortFields } from "../../components/LaunchFields";
import { useT, type I18nKey } from "../../i18n";
import {
  cancelSessionTitle,
  renameSessionWithAgent,
  sessionTitleOptions,
  sessionTitleOptionsMany,
  type BatchSessionTitleOptions,
} from "../../ipc/tree";
import { genId } from "../../genId";
import { useTermStore } from "../../store/termStore";
import type { SessionKind } from "../../types";
import { kindIconEl } from "../sessionViewers/sessionMeta";
import { navigateSmartRename, readSmartRenameRoute, useSmartRenameRoute, writeSmartRenameSelection, type SmartRenameRoute as Route } from "./navigation";
import "./smart-rename.css";

/** Maximum number of sessions renamed in parallel during a batch run. */
const BATCH_CONCURRENCY = 3;

const ERROR_KEYS: Record<string, I18nKey> = {
  agent_unavailable: "sessionTitle.agentUnavailable", empty: "sessionTitle.unavailable",
  unavailable: "sessionTitle.unavailable", unsupported: "sessionTitle.unavailable",
  busy: "sessionTitle.busy", too_large: "sessionTitle.tooLarge", timeout: "sessionTitle.timeout",
  invalid: "sessionTitle.invalid", changed: "sessionTitle.changed",
  invalid_selection: "sessionTitle.invalidSelection",
};
export function titleErrorKey(error: unknown): I18nKey {
  return ERROR_KEYS[String(error).match(/session_title:([a-z_]+)/)?.[1] ?? ""] ?? "sessionTitle.failed";
}

export function SmartRenameRoute() {
  const route = useSmartRenameRoute();
  if (!route) return null;
  if (route.sessionIds.length > 1) {
    return <BatchRenameDialog route={route} key={route.sessionIds.join(",")} />;
  }
  return <AgentChoice route={route} key={route.sessionId} />;
}

function AgentChoice({ route }: { route: Route }) {
  const t = useT();
  const [data, setData] = useState<Awaited<ReturnType<typeof sessionTitleOptions>> | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const active = useRef<{ id: string; promise: Promise<unknown>; cancelling: boolean } | null>(null);
  const close = () => navigateSmartRename(null);
  useEffect(() => () => {
    const task = active.current;
    if (task && !task.cancelling) {
      task.cancelling = true;
      void cancelSessionTitle(route.sessionId, task.id).catch(() => {});
    }
  }, [route.sessionId]);
  useEffect(() => {
    let live = true;
    setData(null); setError(null);
    void sessionTitleOptions(route.sessionId).then(value => { if (live) setData(value); })
      .catch(cause => { if (live) setError(t(titleErrorKey(cause))); });
    return () => { live = false; };
  }, [route.sessionId, revision, t]);
  // Load inside the same centered modal as the form so the dialog does not jump once options arrive.
  // The empty required field keeps submission disabled until the real form replaces this one.
  if (!data) return <FormModal key="loading" title={t("sessionTitle.rename")} description={t("sessionTitle.confirmHint")}
    submitLabel={t("common.rename")} onCancel={close} onSubmit={() => {}}
    fields={[{ key: "loading", label: "", required: true, render: () =>
      <LaunchLoadState state={error ? "error" : "loading"} error={error} retry={() => setRevision(value => value + 1)} /> }]} />;
  const lastAgent = useTermStore.getState().sessionTitleAgent;
  // The agent explicitly chosen last time wins while it is still offered and installed, so a repeated
  // rename opens on the setup that was used last time; otherwise the session's own agent is the default.
  const agent = route.agent ?? (lastAgent && data.agents.some(option => option.id === lastAgent && option.available)
    ? lastAgent : data.agent);
  // An agent's last model and effort chosen here win over the session's own; without one, use the session's.
  const choiceFor = (id: string) => {
    const option = data.agents.find(item => item.id === id);
    const saved = useTermStore.getState().sessionTitlePrefs[id as SessionKind];
    return { model: saved?.model ?? option?.model ?? "", effort: saved?.effort ?? option?.effort ?? "" };
  };
  // Only a value that differs from what the dialog would open on counts as a choice, so tabbing through
  // the fields does not pin the session's current model.
  const remember = (id: string, field: "model" | "effort", value: string) => {
    if (id && value !== choiceFor(id)[field]) useTermStore.getState().setSessionTitlePrefs(id as SessionKind, { [field]: value });
  };
  const defaults = choiceFor(agent);
  return <FormModal key={JSON.stringify([route.sessionId, route.agent, route.model, route.effort])} title={t("sessionTitle.rename")}
    description={t("sessionTitle.confirmHint")} submitLabel={t("common.rename")} submittingLabel={t("sessionTitle.generating")}
    initial={{ agent, model: route.model ?? defaults.model, effort: route.effort ?? defaults.effort }}
    fields={[{ key: "agent", label: t("orch.agentLabel"), required: true, render: (value, _change, context) =>
      <AgentSelect value={value as SessionKind | ""} disabled={context.disabled} onChange={next => {
        context.changeValues({ agent: next, ...choiceFor(next) });
        if (next) useTermStore.getState().setSessionTitleAgent(next);
      }} options={data.agents} placeholder={t("orch.agentLabel")} /> },
      { key: "model", label: "", render: (_value, _change, { values, changeValues, disabled }) => {
        const spec = data.agents.find(option => option.id === values.agent && option.available);
        // ZCode has no per-run model or reasoning switch, so those fields would be a promise it cannot keep.
        return spec && spec.modelSelectable ? <FieldStack><ModelEffortFields spec={spec} model={values.model} effort={values.effort}
          disabled={disabled} context={{ parentSessionId: route.sessionId, inheritArgs: true }}
          onChange={(model, effort) => changeValues({ model, effort })}
          onModelCommit={model => remember(values.agent, "model", model)}
          onEffortCommit={effort => remember(values.agent, "effort", effort)} /></FieldStack> : null;
      }}, { key: "effort", label: "", type: "hidden" }]}
    validate={values => !data.agents.some(option => option.available) ? t("sessionTitle.noAgent")
      : values.agent && !data.agents.some(option => option.id === values.agent && option.available)
        ? t("sessionTitle.agentUnavailable") : null}
    onValuesChange={writeSmartRenameSelection}
    formatSubmitError={cause => t(titleErrorKey(cause))} onCancel={close}
    onCancelSubmit={async () => {
      const task = active.current;
      if (!task) return;
      task.cancelling = true;
      try {
        await cancelSessionTitle(route.sessionId, task.id);
      } catch (cause) {
        task.cancelling = false;
        throw cause;
      }
      // Wait for process cleanup and the session claim to be released before allowing a retry.
      await task.promise.catch(() => {});
    }}
    onSubmit={async values => {
      const id = genId();
      const task = { id, promise: renameSessionWithAgent(route.sessionId, values.agent as SessionKind, values.model, values.effort, id), cancelling: false };
      active.current = task;
      try {
        await task.promise;
        if (!task.cancelling && active.current === task && readSmartRenameRoute()?.sessionId === route.sessionId) navigateSmartRename(null, true);
      } finally {
        if (active.current === task) active.current = null;
      }
    }} />;
}

type RowStatus =
  | { state: "idle" }
  | { state: "pending" }
  | { state: "running" }
  | { state: "done"; newTitle: string }
  | { state: "error"; error: string }
  | { state: "cancelled" };

interface BatchRun {
  cancelling: boolean;
  inFlight: Map<string, { operationId: string; promise: Promise<unknown> }>;
}

function BatchRenameDialog({ route }: { route: Route }) {
  const t = useT();
  const dialogRef = useRef<HTMLDivElement>(null);
  const descriptionId = useId();
  const routeKey = route.sessionIds.join(",");

  const [data, setData] = useState<BatchSessionTitleOptions | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);

  const [values, setValues] = useState<{ agent: string; model: string; effort: string }>({
    agent: route.agent ?? "",
    model: route.model ?? "",
    effort: route.effort ?? "",
  });
  const [checkedIds, setCheckedIds] = useState<Set<string>>(() => new Set(route.sessionIds));
  const [statuses, setStatuses] = useState<Record<string, RowStatus>>({});
  const [running, setRunning] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [hasCompletedRun, setHasCompletedRun] = useState(false);
  const [cancelError, setCancelError] = useState<string | null>(null);

  const activeRun = useRef<BatchRun | null>(null);
  const runningRef = useRef(false);
  const cancellingRef = useRef(false);

  const close = () => navigateSmartRename(null);

  // Focus dialog on mount and restore focus on unmount, matching FormModal.
  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const dialog = dialogRef.current;
    if (dialog && !dialog.contains(document.activeElement)) dialog.focus();
    return () => { if (previous?.isConnected) previous.focus(); };
  }, []);

  // Cancel all in-flight tasks if the user navigates away while a batch is running.
  useEffect(() => () => {
    const run = activeRun.current;
    if (run && !run.cancelling) {
      run.cancelling = true;
      for (const [sid, entry] of run.inFlight.entries()) {
        void cancelSessionTitle(sid, entry.operationId).catch(() => {});
      }
    }
  }, [routeKey]);

  useEffect(() => {
    let live = true;
    setData(null);
    setLoadError(null);
    void sessionTitleOptionsMany(route.sessionIds)
      .then(result => {
        if (!live) return;
        const lastAgent = useTermStore.getState().sessionTitleAgent;
        const resolvedAgent = route.agent
          ?? (lastAgent && result.agents.some(option => option.id === lastAgent && option.available)
            ? lastAgent
            : result.agent);
        const option = result.agents.find(item => item.id === resolvedAgent);
        const saved = useTermStore.getState().sessionTitlePrefs[resolvedAgent as SessionKind];
        const defaultModel = saved?.model ?? option?.model ?? "";
        const defaultEffort = saved?.effort ?? option?.effort ?? "";
        setValues({
          agent: resolvedAgent,
          model: route.model ?? defaultModel,
          effort: route.effort ?? defaultEffort,
        });
        setCheckedIds(new Set(result.sessions.map(s => s.id)));
        setStatuses({});
        setHasCompletedRun(false);
        setData(result);
      })
      .catch(cause => {
        if (live) setLoadError(t(titleErrorKey(cause)));
      });
    return () => { live = false; };
  }, [routeKey, revision, t]);

  const choiceFor = (id: string) => {
    const option = data?.agents.find(item => item.id === id);
    const saved = useTermStore.getState().sessionTitlePrefs[id as SessionKind];
    return { model: saved?.model ?? option?.model ?? "", effort: saved?.effort ?? option?.effort ?? "" };
  };

  const remember = (id: string, field: "model" | "effort", value: string) => {
    if (id && value !== choiceFor(id)[field]) {
      useTermStore.getState().setSessionTitlePrefs(id as SessionKind, { [field]: value });
    }
  };

  const updateValues = (changes: Partial<{ agent: string; model: string; effort: string }>) => {
    if (runningRef.current || cancellingRef.current) return;
    setCancelError(null);
    setValues(prev => {
      const next = { ...prev, ...changes };
      writeSmartRenameSelection(next);
      return next;
    });
  };

  const cancel = () => {
    if (cancellingRef.current) return;
    if (!runningRef.current) {
      close();
      return;
    }
    const run = activeRun.current;
    if (!run) {
      close();
      return;
    }
    run.cancelling = true;
    cancellingRef.current = true;
    setCancelling(true);
    setCancelError(null);
    // Mark queued items as cancelled immediately so the list reflects the stop.
    setStatuses(prev => {
      const next = { ...prev };
      for (const [id, st] of Object.entries(next)) {
        if (st.state === "pending") next[id] = { state: "cancelled" };
      }
      return next;
    });
    const entries = Array.from(run.inFlight.entries());
    void Promise.allSettled(entries.map(([sid, entry]) => cancelSessionTitle(sid, entry.operationId)))
      .then(async results => {
        const failedAll = results.length > 0 && results.every(r => r.status === "rejected");
        if (failedAll) {
          const first = results[0] as PromiseRejectedResult;
          run.cancelling = false;
          setCancelError(t(titleErrorKey(first.reason)));
          return;
        }
        await Promise.allSettled(entries.map(([, entry]) => entry.promise));
        close();
      })
      .finally(() => {
        cancellingRef.current = false;
        setCancelling(false);
      });
  };

  const busy = running || cancelling;
  const sessions = data?.sessions ?? [];
  const validationError = !data
    ? null
    : !data.agents.some(option => option.available)
      ? t("sessionTitle.noAgent")
      : values.agent && !data.agents.some(option => option.id === values.agent && option.available)
        ? t("sessionTitle.agentUnavailable")
        : null;

  const selectedSessions = sessions.filter(s => checkedIds.has(s.id));
  const pendingTargets = selectedSessions.filter(s => statuses[s.id]?.state !== "done");
  const failedTargets = selectedSessions.filter(s => statuses[s.id]?.state === "error");
  const doneCount = sessions.filter(s => statuses[s.id]?.state === "done").length;
  const failedCount = sessions.filter(s => statuses[s.id]?.state === "error").length;

  // Progress counts over the active batch target set (items not idle).
  const activeTotal = sessions.filter(s => {
    const st = statuses[s.id]?.state;
    return st && st !== "idle";
  }).length;
  const completedCount = sessions.filter(s => {
    const st = statuses[s.id]?.state;
    return st === "done" || st === "error" || st === "cancelled";
  }).length;

  const canSubmit = !!data && !busy && !validationError && values.agent.trim().length > 0 && pendingTargets.length > 0;

  const startBatch = async () => {
    if (!canSubmit || runningRef.current || cancellingRef.current) return;
    const targets = pendingTargets.map(s => s.id);
    if (targets.length === 0) return;

    const run: BatchRun = { cancelling: false, inFlight: new Map() };
    activeRun.current = run;
    runningRef.current = true;
    setRunning(true);
    setCancelError(null);

    setStatuses(prev => {
      const next = { ...prev };
      for (const id of targets) next[id] = { state: "pending" };
      return next;
    });

    const queue = [...targets];
    let anyFailed = false;
    const workerCount = Math.min(BATCH_CONCURRENCY, queue.length);
    const worker = async () => {
      while (!run.cancelling) {
        const sid = queue.shift();
        if (!sid) break;
        const opId = genId();
        setStatuses(prev => ({ ...prev, [sid]: { state: "running" } }));
        const promise = renameSessionWithAgent(
          sid,
          values.agent as SessionKind,
          values.model,
          values.effort,
          opId,
        );
        run.inFlight.set(sid, { operationId: opId, promise });
        try {
          const result = await promise;
          if (run.cancelling) {
            setStatuses(prev => ({ ...prev, [sid]: { state: "cancelled" } }));
          } else {
            setStatuses(prev => ({ ...prev, [sid]: { state: "done", newTitle: result.title } }));
          }
        } catch (cause) {
          if (run.cancelling || String(cause).includes("session_title:cancelled")) {
            setStatuses(prev => ({ ...prev, [sid]: { state: "cancelled" } }));
          } else {
            anyFailed = true;
            setStatuses(prev => ({ ...prev, [sid]: { state: "error", error: t(titleErrorKey(cause)) } }));
          }
        } finally {
          run.inFlight.delete(sid);
        }
      }
    };

    try {
      await Promise.all(Array.from({ length: workerCount }, () => worker()));
      if (!run.cancelling && activeRun.current === run) {
        setHasCompletedRun(true);
        if (!anyFailed && readSmartRenameRoute()?.sessionIds.join(",") === routeKey) {
          navigateSmartRename(null, true);
        }
      }
    } finally {
      if (activeRun.current === run) activeRun.current = null;
      runningRef.current = false;
      setRunning(false);
    }
  };

  const toggleAll = () => {
    if (busy || !data) return;
    const selectable = sessions.filter(s => statuses[s.id]?.state !== "done");
    const allSelected = selectable.every(s => checkedIds.has(s.id));
    setCheckedIds(prev => {
      const next = new Set(prev);
      for (const s of selectable) {
        if (allSelected) next.delete(s.id);
        else next.add(s.id);
      }
      return next;
    });
  };

  const toggleOne = (id: string) => {
    if (busy || statuses[id]?.state === "done") return;
    setCheckedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const spec = data?.agents.find(option => option.id === values.agent && option.available);
  const matchingSessionId = sessions.find(s => s.kind === values.agent)?.id ?? sessions[0]?.id ?? null;
  const selectable = sessions.filter(s => statuses[s.id]?.state !== "done");
  const allSelectableChecked = selectable.length > 0 && selectable.every(s => checkedIds.has(s.id));
  const submitLabel = hasCompletedRun && failedTargets.length > 0 && pendingTargets.length === failedTargets.length
    ? t("sessionTitle.batchRetryFailed", failedTargets.length)
    : t("sessionTitle.batchSubmit", pendingTargets.length);

  return (
    <Backdrop onClose={cancel}>
      <div
        ref={dialogRef}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-label={t("sessionTitle.batchRename")}
        aria-describedby={descriptionId}
        aria-busy={busy}
        className="smart-rename-dialog"
        onClick={e => e.stopPropagation()}
        onKeyDown={e => {
          if (e.defaultPrevented) return;
          if (e.key === "Tab") {
            const nodes = Array.from(
              dialogRef.current?.querySelectorAll<HTMLElement>(
                'input:not(:disabled), button:not(:disabled), textarea:not(:disabled), a[href], [tabindex="0"]',
              ) ?? [],
            );
            const first = nodes[0];
            const last = nodes[nodes.length - 1];
            if (e.shiftKey && (document.activeElement === first || document.activeElement === dialogRef.current)) {
              e.preventDefault();
              (last ?? dialogRef.current)?.focus();
            } else if (!e.shiftKey && (document.activeElement === last || document.activeElement === dialogRef.current)) {
              e.preventDefault();
              (first ?? dialogRef.current)?.focus();
            }
          }
          if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            cancel();
          }
        }}
      >
        <div className="smart-rename-head">
          <div className="smart-rename-title">{t("sessionTitle.batchRename")}</div>
          <p id={descriptionId} className="smart-rename-desc">{t("sessionTitle.batchConfirmHint")}</p>
        </div>

        <div className="smart-rename-body">
          {!data ? (
            <LaunchLoadState
              state={loadError ? "error" : "loading"}
              error={loadError}
              retry={() => setRevision(value => value + 1)}
            />
          ) : (
            <>
              <FieldStack>
                <Field as="div" label={t("orch.agentLabel")} required>
                  <AgentSelect
                    value={values.agent as SessionKind | ""}
                    disabled={busy}
                    onChange={next => {
                      updateValues({ agent: next, ...choiceFor(next) });
                      if (next) useTermStore.getState().setSessionTitleAgent(next);
                    }}
                    options={data.agents}
                    placeholder={t("orch.agentLabel")}
                  />
                </Field>
                {spec && spec.modelSelectable && (
                  <ModelEffortFields
                    spec={spec}
                    model={values.model}
                    effort={values.effort}
                    disabled={busy}
                    context={{ parentSessionId: matchingSessionId, inheritArgs: true }}
                    onChange={(model, effort) => updateValues({ model, effort })}
                    onModelCommit={model => remember(values.agent, "model", model)}
                    onEffortCommit={effort => remember(values.agent, "effort", effort)}
                  />
                )}
              </FieldStack>

              <div className="smart-rename-section">
                <div className="smart-rename-section-head">
                  <span className="smart-rename-section-label">
                    {t("sessionTitle.batchSessions", selectedSessions.length, sessions.length)}
                  </span>
                  {selectable.length > 1 && (
                    <button
                      type="button"
                      className="smart-rename-toggle"
                      disabled={busy}
                      onClick={toggleAll}
                    >
                      {allSelectableChecked ? t("sessionTitle.batchDeselectAll") : t("sessionTitle.batchSelectAll")}
                    </button>
                  )}
                </div>

                <div className="smart-rename-list" role="list">
                  {sessions.map(item => {
                    const st = statuses[item.id] ?? { state: "idle" as const };
                    const checked = checkedIds.has(item.id);
                    return (
                      <div key={item.id} className="smart-rename-row" role="listitem">
                        <input
                          type="checkbox"
                          className="smart-rename-row-check"
                          checked={checked}
                          disabled={busy || st.state === "done"}
                          aria-label={item.name}
                          onChange={() => toggleOne(item.id)}
                        />
                        <span className="smart-rename-row-icon" aria-hidden="true">
                          {kindIconEl(item.kind, 13)}
                        </span>
                        <div className="smart-rename-row-main">
                          <span
                            className={`smart-rename-row-name${st.state === "done" ? " done" : ""}`}
                            title={item.name}
                          >
                            {item.name}
                          </span>
                          {st.state === "done" && (
                            <span className="smart-rename-row-new" title={st.newTitle}>
                              {st.newTitle}
                            </span>
                          )}
                          {st.state === "error" && (
                            <span className="smart-rename-row-error" role="alert">
                              {st.error}
                            </span>
                          )}
                        </div>
                        {st.state !== "idle" && (
                          <span className={`smart-rename-row-status ${st.state}`}>
                            {st.state === "pending" && t("sessionTitle.batchQueued")}
                            {st.state === "running" && t("sessionTitle.generating")}
                            {st.state === "done" && <Icons.check size={13} />}
                            {st.state === "error" && <Icons.warn size={13} />}
                            {st.state === "cancelled" && t("sessionTitle.batchCancelled")}
                          </span>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>

              {hasCompletedRun && !running && failedCount > 0 && (
                <div className="smart-rename-summary has-error" role="status">
                  {t("sessionTitle.batchSummary", doneCount, failedCount)}
                </div>
              )}
            </>
          )}
        </div>

        {(validationError || cancelError) && (
          <div role="alert" className="smart-rename-error">
            {validationError ?? cancelError}
          </div>
        )}

        <div className="smart-rename-foot">
          <button type="button" className="vlx-btn" onClick={cancel} disabled={cancelling}>
            {hasCompletedRun && !running ? t("common.close") : t("common.cancel")}
          </button>
          <button
            type="button"
            className="vlx-btn vlx-btn-primary"
            onClick={() => void startBatch()}
            disabled={!canSubmit}
          >
            {running ? (
              <span role="status">
                {t("sessionTitle.batchProgress", completedCount, activeTotal)}
              </span>
            ) : (
              submitLabel
            )}
          </button>
        </div>
      </div>
    </Backdrop>
  );
}
