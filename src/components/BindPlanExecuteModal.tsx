//! Bind existing chat conversations into a persistent Plan/Execute workflow.

import { useEffect, useId, useMemo, useRef, useState } from "react";
import { useT } from "../i18n";
import { dataUrl } from "../layout/CenterPane/session/attachments";
import { kindIconEl } from "../layout/sessionViewers/sessionMeta";
import {
  applyLaunchArgs,
  bindPlanExecute,
  planExecuteDefaults,
  preparePlanExecuteBind,
  type BindSessionCandidate,
  type PlanExecuteBindPreparation,
  type PlanExecuteConfig,
} from "../ipc/launch";
import type { WorktreeMode } from "../ipc/events";
import { useTermStore } from "../store/termStore";
import type { SessionKind } from "../types";
import AgentSelect from "./AgentSelect";
import { Backdrop } from "./Backdrop";
import { Icons } from "./Icons";
import {
  LaunchCloseButton,
  LaunchField,
  LaunchLoadState,
  ModelEffortFields,
  WorktreeChoices,
  launchErrorText,
  useLaunchOptions,
} from "./LaunchFields";
import Select, { type SelectOption } from "./Select";
import {
  bindPlanExecuteUrl,
  navigateBindPlanExecute,
  useBindPlanExecuteRoute,
} from "./planExecuteNavigation";
import { useTaskImages } from "./useTaskImages";

const MAX_BOUND_EXECUTORS = 12;

function shortenCwd(cwd: string | null): string | undefined {
  if (!cwd) return undefined;
  const trimmed = cwd.trim();
  if (!trimmed) return undefined;
  const parts = trimmed.split(/[\\/]/).filter(Boolean);
  if (parts.length <= 2) return trimmed;
  return `…/${parts.slice(-2).join("/")}`;
}

function clampWorktreeMode(mode: WorktreeMode | undefined): WorktreeMode {
  return mode === "each" ? "each" : "none";
}

export function BindPlanExecuteModal() {
  const t = useT();
  const titleId = useId();
  const route = useBindPlanExecuteRoute();
  const groups = useTermStore((s) => s.groups);
  const projects = useTermStore((s) => s.projects);

  const catalog = useLaunchOptions(Boolean(route));
  const [prep, setPrep] = useState<PlanExecuteBindPreparation | null>(null);
  const [prepError, setPrepError] = useState<string | null>(null);
  const [prepRevision, setPrepRevision] = useState(0);

  const [plannerId, setPlannerId] = useState("");
  const [executorIds, setExecutorIds] = useState<string[]>([]);
  const [reparentExecutors, setReparentExecutors] = useState(true);
  const [prompt, setPrompt] = useState("");
  const [config, setConfig] = useState<PlanExecuteConfig>({
    reviewEnabled: false,
    worktreeMode: "none",
    splitTasks: false,
    plan: {},
    exec: {},
    review: {},
  });
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const errorEl = useRef<HTMLParagraphElement | null>(null);

  const taskImages = useTaskImages(route ?? undefined, []);

  useEffect(() => {
    if (error) errorEl.current?.scrollIntoView?.({ block: "nearest" });
  }, [error]);

  const routeKey = route ? JSON.stringify(route) : "";

  useEffect(() => {
    if (!route) {
      setPrep(null);
      setPrepError(null);
      setSubmitting(false);
      setError(null);
      return;
    }
    let live = true;
    setPrep(null);
    setPrepError(null);
    setError(null);
    setSubmitting(false);
    setPrompt("");
    setReparentExecutors(true);

    void preparePlanExecuteBind(route.sessionIds, route.role === "exec" ? "executors" : undefined)
      .then((result) => {
        if (!live) return;
        setPrep(result);
        const byId = new Map(result.candidates.map((c) => [c.id, c]));
        let nextPlanner = result.plannerId;
        let nextExecs = result.executorIds;
        // Preserve explicit deep-link command + executor selection when both are provided in URL.
        if (route.plannerId && route.executorIds.length > 0 && byId.has(route.plannerId)) {
          nextPlanner = route.plannerId;
          const plannerCand = byId.get(nextPlanner);
          const cap = plannerCand?.existingRun?.remainingSlots ?? MAX_BOUND_EXECUTORS;
          nextExecs = route.executorIds
            .filter((id) => {
              const cand = byId.get(id);
              return id !== nextPlanner && !!cand && cand.eligible && !cand.activeRole;
            })
            .slice(0, cap);
        }
        setPlannerId(nextPlanner);
        setExecutorIds(nextExecs);
        const initialConfig =
          (nextPlanner ? byId.get(nextPlanner)?.existingRun?.config : undefined) ??
          result.defaultConfig;
        setConfig({
          ...initialConfig,
          reviewEnabled: initialConfig.reviewEnabled ?? false,
          worktreeMode: clampWorktreeMode(initialConfig.worktreeMode),
          splitTasks: initialConfig.splitTasks ?? false,
          plan: initialConfig.plan ?? {},
          exec: initialConfig.exec ?? {},
          review: initialConfig.review ?? {},
        });
      })
      .catch((cause) => {
        if (live) setPrepError(launchErrorText(cause));
      });
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [routeKey, prepRevision]);

  const candidates = useMemo(() => prep?.candidates ?? [], [prep]);
  const candidateById = useMemo(
    () => new Map(candidates.map((c) => [c.id, c])),
    [candidates],
  );
  const selectedPlanner = plannerId ? candidateById.get(plannerId) ?? null : null;
  const isExtendMode = Boolean(
    selectedPlanner?.extendablePlanner && selectedPlanner.existingRun,
  );
  const maxExecutors = isExtendMode
    ? selectedPlanner!.existingRun!.remainingSlots
    : MAX_BOUND_EXECUTORS;

  const groupNameById = useMemo(
    () => new Map(groups.map((g) => [g.id, g.name])),
    [groups],
  );
  const projectNameById = useMemo(
    () => new Map(projects.map((p) => [p.id, p.name])),
    [projects],
  );

  const candidateHint = (c: BindSessionCandidate): string | undefined => {
    const parts: string[] = [];
    if (selectedPlanner && c.projectId !== selectedPlanner.projectId) {
      const proj = projectNameById.get(c.projectId);
      if (proj) parts.push(proj);
    }
    if (c.groupId) {
      const grp = groupNameById.get(c.groupId);
      if (grp) parts.push(grp);
    }
    const dir = shortenCwd(c.worktreePath ?? c.cwd);
    if (dir && parts.length === 0) parts.push(dir);
    return parts.length ? parts.join(" · ") : undefined;
  };

  const disabledReasonTag = (c: BindSessionCandidate, forPlan: boolean): string | undefined => {
    if (!c.eligible) {
      return t(
        c.ineligibleReason === "not_chat"
          ? "bindPlanExecute.tagNotChat"
          : "bindPlanExecute.tagUnsupported",
      );
    }
    if (c.activeRole) {
      if (forPlan && c.extendablePlanner) return t("bindPlanExecute.tagExtendable");
      return t("bindPlanExecute.tagInWorkflow");
    }
    return undefined;
  };

  const plannerOptions = useMemo<SelectOption<string>[]>(
    () =>
      candidates.map((c) => {
        const disabled = !c.eligible || (c.activeRole !== null && !c.extendablePlanner);
        return {
          value: c.id,
          label: c.name,
          icon: kindIconEl(c.kind, 14),
          hint: candidateHint(c),
          tag: disabledReasonTag(c, true),
          disabled,
        };
      }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [candidates, selectedPlanner, groupNameById, projectNameById, t],
  );

  const executorOptions = useMemo<SelectOption<string>[]>(() => {
    const others = candidates.filter((c) => c.id !== plannerId);
    const children = others.filter((c) => c.parentSessionId === plannerId);
    const rest = others.filter((c) => c.parentSessionId !== plannerId);
    const ordered = [...children, ...rest];
    return ordered.map((c) => {
      const disabled = !c.eligible || c.activeRole !== null;
      const reason = disabledReasonTag(c, false);
      const isChild = Boolean(plannerId && c.parentSessionId === plannerId);
      return {
        value: c.id,
        label: c.name,
        icon: kindIconEl(c.kind, 14),
        hint: candidateHint(c),
        tag: reason ?? (isChild ? t("bindPlanExecute.tagChild") : undefined),
        disabled,
      };
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [candidates, plannerId, selectedPlanner, groupNameById, projectNameById, t]);

  const hasReparentableExecutors = useMemo(() => {
    if (!selectedPlanner) return false;
    return executorIds.some((id) => {
      const c = candidateById.get(id);
      return (
        !!c &&
        c.projectId === selectedPlanner.projectId &&
        c.parentSessionId !== selectedPlanner.id
      );
    });
  }, [selectedPlanner, executorIds, candidateById]);

  const handlePlannerChange = (nextId: string) => {
    setPlannerId(nextId);
    setError(null);
    setExecutorIds((prev) => prev.filter((id) => id !== nextId));
    const nextCand = candidateById.get(nextId);
    if (!nextCand) return;
    if (nextCand.existingRun) {
      setConfig({
        ...nextCand.existingRun.config,
        worktreeMode: clampWorktreeMode(nextCand.existingRun.config.worktreeMode),
      });
      return;
    }
    if (nextCand.eligible) {
      void planExecuteDefaults(nextId, {
        reviewEnabled: config.reviewEnabled ?? false,
        worktreeMode: clampWorktreeMode(config.worktreeMode),
        splitTasks: config.splitTasks ?? false,
        plan: {},
        exec: config.exec ?? {},
        review: config.review ?? {},
      })
        .then((resolved) => {
          setConfig((prev) => ({
            ...resolved,
            reviewEnabled: prev.reviewEnabled ?? resolved.reviewEnabled ?? false,
            worktreeMode: clampWorktreeMode(prev.worktreeMode),
            splitTasks: prev.splitTasks ?? false,
          }));
        })
        .catch(() => {});
    }
  };

  if (!route) return null;

  const close = () => navigateBindPlanExecute(bindPlanExecuteUrl(null));

  const plannerValid = Boolean(
    selectedPlanner &&
      selectedPlanner.eligible &&
      (selectedPlanner.activeRole === null || selectedPlanner.extendablePlanner),
  );
  const execCountExceeded = executorIds.length > maxExecutors;
  const extendMissingExec = isExtendMode && executorIds.length === 0;
  const availableAgents = catalog.options.filter((o) => o.supportsPlanExecute);
  const execSpec = availableAgents.find((o) => o.id === config.exec?.agent);
  const reviewSpec = availableAgents.find((o) => o.id === config.review?.agent);
  const needsExecConfig = !isExtendMode && executorIds.length === 0;
  const needsReviewConfig = !isExtendMode && config.reviewEnabled === true;

  const rolesReady =
    catalog.state === "ready" &&
    (!needsExecConfig || Boolean(execSpec)) &&
    (!needsReviewConfig || Boolean(reviewSpec));

  const canSubmit =
    !submitting &&
    Boolean(prep) &&
    plannerValid &&
    !execCountExceeded &&
    !extendMissingExec &&
    rolesReady &&
    !taskImages.busy;

  const execSummary =
    executorIds.length === 0
      ? undefined
      : executorIds.length === 1
        ? candidateById.get(executorIds[0])?.name
        : t("bindPlanExecute.execSummary", executorIds.length);

  const execHint = isExtendMode
    ? t("bindPlanExecute.execHintExtend", maxExecutors)
    : executorIds.length === 0
      ? t("bindPlanExecute.execHintNone")
      : executorIds.length === 1
        ? t("bindPlanExecute.execHintSingle")
        : t("bindPlanExecute.execHintMultiple", executorIds.length);

  const submit = async () => {
    if (!canSubmit || !selectedPlanner) return;
    setSubmitting(true);
    setError(null);
    try {
      if (needsExecConfig && config.exec?.agent) {
        await applyLaunchArgs(config.exec.agent, null, config.exec.model, config.exec.effort);
      }
      if (needsReviewConfig && config.review?.agent) {
        await applyLaunchArgs(
          config.review.agent,
          null,
          config.review.model,
          config.review.effort,
        );
      }
      const images = taskImages.attachments.map(({ mimeType, data }) => ({ mimeType, data }));
      const payloadConfig: PlanExecuteConfig = isExtendMode
        ? selectedPlanner.existingRun!.config
        : {
            reviewEnabled: config.reviewEnabled === true,
            worktreeMode:
              executorIds.length === 0 ? clampWorktreeMode(config.worktreeMode) : "none",
            splitTasks: executorIds.length === 0 ? Boolean(config.splitTasks) : executorIds.length >= 2,
            plan: config.plan ?? {},
            exec: config.exec ?? {},
            ...(config.reviewEnabled === true ? { review: config.review ?? {} } : {}),
          };
      const result = await bindPlanExecute({
        requestId: route.requestId,
        plannerId: selectedPlanner.id,
        executorIds,
        prompt,
        ...(images.length > 0 ? { images } : {}),
        reparentExecutors: hasReparentableExecutors && reparentExecutors,
        config: payloadConfig,
      });
      await useTermStore.getState().loadTree();
      if (result.run.state === "blocked" || result.delivery?.delivery === "blocked") {
        throw new Error(
          result.delivery?.error || result.run.summary || t("bindPlanExecute.deliveryBlocked"),
        );
      }
      navigateBindPlanExecute(bindPlanExecuteUrl(null));
      useTermStore.getState().openSession(result.planner.id);
    } catch (cause) {
      setError(launchErrorText(cause));
      setSubmitting(false);
    }
  };

  return (
    <Backdrop onClose={close} center={false}>
      <section
        className="launch-dialog launch-single"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        onKeyDown={(event) => {
          if (event.key === "Escape" && !event.defaultPrevented) {
            event.preventDefault();
            close();
          } else if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
            event.preventDefault();
            void submit();
          }
        }}
      >
        <header className="launch-header">
          <div>
            <h2 id={titleId}>{t("bindPlanExecute.title")}</h2>
            <p>{t("bindPlanExecute.intro")}</p>
          </div>
          <div className="launch-header-actions">
            <LaunchCloseButton onClose={close} />
          </div>
        </header>

        <div className="launch-body">
          {!prep && (
            <LaunchLoadState
              state={prepError ? "error" : "loading"}
              error={prepError}
              retry={() => setPrepRevision((r) => r + 1)}
            />
          )}

          {prep && (
            <>
              <LaunchField
                as="div"
                label={t("bindPlanExecute.planSessionLabel")}
                hint={t("bindPlanExecute.planSessionHint")}
              >
                <Select
                  value={plannerId}
                  onChange={handlePlannerChange}
                  options={plannerOptions}
                  width="100%"
                  menuPortal
                  searchable
                  placeholder={t("bindPlanExecute.planPlaceholder")}
                  searchPlaceholder={t("chat.filterPlaceholder")}
                  noMatchLabel={t("tree.noMatch")}
                  disabled={submitting}
                  ariaLabel={t("bindPlanExecute.planSessionLabel")}
                />
              </LaunchField>

              {!plannerValid && (
                <p
                  className={plannerId ? "launch-error" : "launch-hint"}
                  role={plannerId ? "alert" : "status"}
                >
                  {t("bindPlanExecute.plannerRequired")}
                </p>
              )}

              {isExtendMode && selectedPlanner?.existingRun && (
                <div className="launch-notice" role="status">
                  <div>
                    <strong>
                      {t(
                        "bindPlanExecute.activeWorkflowNotice",
                        selectedPlanner.existingRun.taskCount,
                        selectedPlanner.existingRun.remainingSlots,
                      )}
                    </strong>
                    <p>
                      {t(
                        selectedPlanner.existingRun.reviewEnabled === true
                          ? "bindPlanExecute.lockedReviewEnabled"
                          : selectedPlanner.existingRun.reviewEnabled === false
                            ? "bindPlanExecute.lockedReviewDisabled"
                            : "bindPlanExecute.lockedReviewLegacy",
                      )}
                    </p>
                  </div>
                </div>
              )}

              <LaunchField
                as="div"
                label={t("bindPlanExecute.execSessionsLabel")}
                hint={execHint}
              >
                <Select
                  multiple
                  value={executorIds}
                  onChange={(next) => {
                    setExecutorIds(next);
                    setError(null);
                  }}
                  options={executorOptions}
                  width="100%"
                  menuPortal
                  searchable
                  searchPlaceholder={t("chat.filterPlaceholder")}
                  noMatchLabel={t("tree.noMatch")}
                  selectAllLabel={t("common.selectAll")}
                  clearLabel={t("importSessions.clearSelection")}
                  placeholder={t("bindPlanExecute.execPlaceholder")}
                  summary={execSummary}
                  disabled={submitting}
                  ariaLabel={t("bindPlanExecute.execSessionsLabel")}
                />
              </LaunchField>

              {execCountExceeded && (
                <p className="launch-error" role="alert">
                  {t("bindPlanExecute.execLimitError", maxExecutors)}
                </p>
              )}

              {extendMissingExec && (
                <p className="launch-hint" role="status">
                  {t("bindPlanExecute.extendExecRequired")}
                </p>
              )}

              {hasReparentableExecutors && (
                <label className="launch-panel launch-split-option">
                  <span className="launch-inline">
                    <input
                      type="checkbox"
                      disabled={submitting}
                      checked={reparentExecutors}
                      onChange={(e) => setReparentExecutors(e.target.checked)}
                    />
                    <strong>{t("bindPlanExecute.reparentLabel")}</strong>
                  </span>
                  <span className="launch-hint">{t("bindPlanExecute.reparentHint")}</span>
                </label>
              )}

              {!isExtendMode && (
                <div className="launch-role-panels">
                  <LaunchLoadState
                    state={catalog.state}
                    error={catalog.error}
                    retry={catalog.retry}
                  />

                  {catalog.state === "ready" && (
                    <>
                      {executorIds.length === 0 && (
                        <>
                          <label className="launch-panel launch-split-option">
                            <span className="launch-inline">
                              <input
                                type="checkbox"
                                disabled={submitting}
                                checked={Boolean(config.splitTasks)}
                                onChange={(e) =>
                                  setConfig((prev) => ({
                                    ...prev,
                                    splitTasks: e.target.checked,
                                  }))
                                }
                              />
                              <strong>{t("launch.splitTasks")}</strong>
                            </span>
                            <span className="launch-hint">{t("launch.splitTasksHint")}</span>
                          </label>

                          <section className="launch-panel" aria-label={t("launch.execTitle")}>
                            <h3>{t("launch.execTitle")}</h3>
                            <LaunchField as="div" label={t("spawn.agentLabel")}>
                              <AgentSelect
                                key={`exec:${String(submitting)}`}
                                disabled={submitting}
                                value={(config.exec?.agent ?? "") as SessionKind | ""}
                                width="100%"
                                ariaLabel={t("spawn.agentLabel")}
                                options={availableAgents.map((option) => ({
                                  id: option.id as SessionKind,
                                  label: option.label,
                                }))}
                                onChange={(agent) => {
                                  if (!agent) return;
                                  setConfig((prev) => ({
                                    ...prev,
                                    exec: { agent: agent as NonNullable<PlanExecuteConfig["exec"]>["agent"], model: "", effort: "" },
                                  }));
                                }}
                              />
                            </LaunchField>
                            {execSpec && (
                              <div className="launch-model-grid">
                                <ModelEffortFields
                                  disabled={submitting}
                                  spec={execSpec}
                                  model={config.exec?.model ?? ""}
                                  effort={config.exec?.effort ?? ""}
                                  context={{
                                    parentSessionId: selectedPlanner?.id,
                                    cwd: selectedPlanner?.worktreePath ?? selectedPlanner?.cwd,
                                  }}
                                  onChange={(model, effort) =>
                                    setConfig((prev) => ({
                                      ...prev,
                                      exec: { ...prev.exec, model, effort },
                                    }))
                                  }
                                />
                              </div>
                            )}
                          </section>

                          <WorktreeChoices
                            allowShared={false}
                            workflow
                            disabled={submitting}
                            value={clampWorktreeMode(config.worktreeMode)}
                            onChange={(mode) =>
                              setConfig((prev) => ({
                                ...prev,
                                worktreeMode: clampWorktreeMode(mode),
                              }))
                            }
                          />
                        </>
                      )}

                      <label className="launch-panel launch-split-option">
                        <span className="launch-inline">
                          <input
                            type="checkbox"
                            disabled={submitting}
                            checked={config.reviewEnabled === true}
                            aria-label={t("launch.reviewEnabled")}
                            onChange={(e) => {
                              const enabled = e.target.checked;
                              setConfig((prev) => ({
                                ...prev,
                                reviewEnabled: enabled,
                                review:
                                  enabled && !prev.review?.agent
                                    ? {
                                        agent: (selectedPlanner?.kind ??
                                          prev.plan?.agent ??
                                          "claude") as NonNullable<PlanExecuteConfig["review"]>["agent"],
                                        model: "",
                                        effort: "",
                                      }
                                    : prev.review,
                              }));
                            }}
                          />
                          <strong>{t("launch.reviewEnabled")}</strong>
                        </span>
                        <span className="launch-hint">
                          {t(
                            config.reviewEnabled === true
                              ? "launch.reviewEnabledHint"
                              : "launch.reviewDisabledHint",
                          )}
                        </span>
                      </label>

                      {config.reviewEnabled === true && (
                        <section className="launch-panel" aria-label={t("launch.reviewTitle")}>
                          <h3>{t("launch.reviewTitle")}</h3>
                          <LaunchField as="div" label={t("spawn.agentLabel")}>
                            <AgentSelect
                              key={`review:${String(submitting)}`}
                              disabled={submitting}
                              value={(config.review?.agent ?? "") as SessionKind | ""}
                              width="100%"
                              ariaLabel={t("spawn.agentLabel")}
                              options={availableAgents.map((option) => ({
                                id: option.id as SessionKind,
                                label: option.label,
                              }))}
                              onChange={(agent) => {
                                if (!agent) return;
                                setConfig((prev) => ({
                                  ...prev,
                                  review: {
                                    agent: agent as NonNullable<PlanExecuteConfig["review"]>["agent"],
                                    model: "",
                                    effort: "",
                                  },
                                }));
                              }}
                            />
                          </LaunchField>
                          {reviewSpec && (
                            <div className="launch-model-grid">
                              <ModelEffortFields
                                disabled={submitting}
                                spec={reviewSpec}
                                model={config.review?.model ?? ""}
                                effort={config.review?.effort ?? ""}
                                context={{
                                  parentSessionId: selectedPlanner?.id,
                                  cwd: selectedPlanner?.worktreePath ?? selectedPlanner?.cwd,
                                }}
                                onChange={(model, effort) =>
                                  setConfig((prev) => ({
                                    ...prev,
                                    review: { ...prev.review, model, effort },
                                  }))
                                }
                              />
                            </div>
                          )}
                        </section>
                      )}
                    </>
                  )}
                </div>
              )}

              <LaunchField
                label={t("bindPlanExecute.promptLabel")}
                hint={t("bindPlanExecute.promptHint")}
              >
                <textarea
                  className="vlx-input launch-task"
                  rows={4}
                  onPaste={submitting ? undefined : taskImages.onPaste}
                  disabled={submitting}
                  value={prompt}
                  onChange={(e) => setPrompt(e.target.value)}
                />
              </LaunchField>

              {taskImages.attachments.length > 0 && (
                <div className="launch-images">
                  {taskImages.attachments.map((image) => (
                    <div className="launch-image" key={image.id} title={image.name}>
                      <img src={dataUrl(image)} alt={image.name} />
                      <button
                        type="button"
                        className="launch-image-remove"
                        title={t("chat.attach.remove")}
                        disabled={submitting || taskImages.busy}
                        onClick={() => taskImages.remove(image.id)}
                      >
                        <Icons.close size={12} />
                      </button>
                    </div>
                  ))}
                </div>
              )}
              {taskImages.busy && (
                <p className="launch-hint" role="status">
                  {t("common.loading")}
                </p>
              )}
              {taskImages.note && (
                <p className="launch-error" role="alert">
                  {taskImages.note}
                </p>
              )}
            </>
          )}

          {error && (
            <p ref={errorEl} className="launch-error" role="alert">
              {error}
            </p>
          )}
        </div>

        <footer className="launch-footer">
          <span className="launch-hint">{t("bindPlanExecute.footerHint")}</span>
          <div className="launch-actions">
            <button
              type="button"
              className="vlx-btn"
              disabled={submitting}
              onClick={close}
            >
              {t("common.cancel")}
            </button>
            <button
              type="button"
              className="vlx-btn vlx-btn-primary"
              disabled={!canSubmit}
              onClick={() => void submit()}
            >
              {t(
                submitting
                  ? "bindPlanExecute.binding"
                  : isExtendMode
                    ? "bindPlanExecute.submitExtend"
                    : "bindPlanExecute.submit",
              )}
            </button>
          </div>
        </footer>
      </section>
    </Backdrop>
  );
}
