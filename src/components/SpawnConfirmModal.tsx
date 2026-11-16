//! Review one child-session request without taking focus away from the current session.

import { useEffect, useId, useMemo, useRef, useState } from "react";
import { useT } from "../i18n";
import { isMobileView } from "../mobile/detect";
import { mobileSessionUrl } from "../mobile/sessionNavigation";
import { useSuspendNativeViews } from "../hooks/nativeViewSuspend";
import { applyLaunchArgs, createPlanExecute, preparePlanExecute, type PlanExecutePreparation } from "../ipc/launch";
import { prepareSpawn } from "../ipc/commands";
import type { SpawnRequest, WorktreeMode } from "../ipc/events";
import { useTermStore } from "../store/termStore";
import {
  launchErrorText,
  ModelEffortFields,
  LaunchCloseButton,
  LaunchField,
  LaunchLoadState,
  useLaunchOptions,
  WorktreeChoices,
} from "./LaunchFields";
import type { SessionKind } from "../types";
import AgentSelect from "./AgentSelect";
import { useTaskImages } from "./useTaskImages";
import { dataUrl } from "../layout/CenterPane/session/attachments";
import { Icons } from "./Icons";
import { PlanExecuteFields } from "./PlanExecuteFields";
import { navigatePlanExecute, planExecuteUrl, usePlanExecuteMenuRoute } from "./planExecuteNavigation";

export function SpawnConfirmModal() {
  const t = useT();
  const titleId = useId();
  const queue = useTermStore((s) => s.pendingSpawns);
  const sessions = useTermStore((s) => s.sessions);
  const projects = useTermStore((s) => s.projects);
  const receipts = useTermStore((s) => s.spawnReceipts);
  const confirmSpawn = useTermStore((s) => s.confirmSpawn);
  const cancelSpawn = useTermStore((s) => s.cancelSpawn);
  const menu = usePlanExecuteMenuRoute();
  const menuRequest = useMemo<SpawnRequest | null>(() => menu ? {
    requestId: menu.requestId, parentSessionId: menu.context.parentSessionId ?? "", prompt: "",
    worktree: false, planExecute: { plan: {}, exec: {} },
  } : null, [menu]);
  const [routeId, setRouteId] = useState(() => new URL(window.location.href).searchParams.get("spawnRequest"));
  const [inspectedRequests, setInspectedRequests] = useState<Set<string>>(() => new Set());
  useEffect(() => {
    const update = () => setRouteId(new URL(window.location.href).searchParams.get("spawnRequest"));
    window.addEventListener("popstate", update);
    return () => window.removeEventListener("popstate", update);
  }, []);
  const req = menuRequest ?? (routeId ? queue.find((item) => item.requestId === routeId)
    : queue.find((item) => !item.requestId || !inspectedRequests.has(item.requestId)));
  useEffect(() => {
    if (menu) return;
    const id = req?.requestId;
    if (!routeId && id) {
      const url = new URL(window.location.href); url.searchParams.set("spawnRequest", id);
      window.history.replaceState(window.history.state, "", url); setRouteId(id);
    } else if (routeId && receipts[routeId] && !queue.some((item) => item.requestId === routeId)) {
      const url = new URL(window.location.href); url.searchParams.delete("spawnRequest");
      window.history.replaceState(window.history.state, "", url); setRouteId(null);
    }
  }, [menu, req?.requestId, routeId, queue, receipts]);
  const receipt = req?.requestId ? receipts[req.requestId] : undefined;
  const accepted = receipt?.decision === "confirmed";

  // Pending receipts refresh independently of the user's local draft. Confirmation replaces that
  // draft once with the approved request, including attachments received from another client.
  const draftKey = accepted ? JSON.stringify(receipt.request) : req?.requestId ?? req;
  const draft = useMemo(() => accepted ? receipt.request : req,
    // The key deliberately excludes background copies of an unconfirmed request.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [draftKey, accepted]);
  const taskImages = useTaskImages(draft, draft?.images);
  const [preparation, setPreparation] = useState<PlanExecutePreparation | null>(null);
  const [preparationError, setPreparationError] = useState<string | null>(null);
  const [preparationRevision, setPreparationRevision] = useState(0);
  const [directory, setDirectory] = useState("");
  useEffect(() => {
    if (!menu) return;
    let live = true;
    setPreparation(null); setPreparationError(null);
    void preparePlanExecute(menu.context).then(value => {
      if (live) { setPreparation(value); setDirectory(value.cwd ?? ""); setWorktree(value.config.worktreeMode ?? (value.worktree ? "shared" : "none")); }
    }).catch(cause => { if (live) setPreparationError(launchErrorText(cause)); });
    return () => { live = false; };
  }, [menu, preparationRevision]);
  const parent = sessions.find((s) => s.id === req?.parentSessionId);
  const catalog = useLaunchOptions(Boolean(req));
  const requestedKind = draft?.kind || "";
  const [prompt, setPrompt] = useState("");
  const [kind, setKind] = useState<string>(requestedKind);
  const [worktree, setWorktree] = useState<WorktreeMode>("none");
  const [model, setModel] = useState("");
  const [effort, setEffort] = useState("");
  const [selectionReady, setSelectionReady] = useState(false);
  const [selectionError, setSelectionError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);
  const preparedSelection = useRef<{ draft: SpawnRequest; kind: string; revision: number } | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const errorElement = useRef<HTMLParagraphElement | null>(null);
  useEffect(() => { if (error) errorElement.current?.scrollIntoView?.({ block: "nearest" }); }, [error]);
  const [workflow, setWorkflow] = useState<SpawnRequest["planExecute"]>(null);
  useEffect(() => {
    if (!draft) return;
    setPrompt(draft.prompt);
    setKind(requestedKind);
    setWorktree(draft.planExecute?.worktreeMode ?? (draft.worktree !== false ? (draft.planExecute ? "shared" : "each") : "none"));
    setError(null);
    setSubmitting(false);
  }, [draft, requestedKind]);
  useEffect(() => {
    if (!draft || draft.planExecute) return;
    if (preparedSelection.current?.draft === draft && preparedSelection.current.kind === kind && preparedSelection.current.revision === revision) return;
    let live = true;
    setSelectionReady(false);
    setSelectionError(null);
    if (!draft.requestId) return;
    void prepareSpawn(draft.requestId, accepted ? undefined : kind || undefined)
      .then((selection) => {
        if (!live) return;
        preparedSelection.current = { draft, kind: selection.kind, revision };
        setKind(selection.kind);
        setModel(selection.model ?? "");
        setEffort(selection.effort ?? "");
        setDirectory(selection.cwd ?? "");
        setSelectionReady(true);
      })
      .catch(cause => {
        if (live) setSelectionError(launchErrorText(cause));
      });
    return () => {
      live = false;
    };
    // Only a new request or user-selected agent resets the draft, not background session updates.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [draft, kind, revision]);
  useSuspendNativeViews(Boolean(req));
  if (!req) return null;
  const spec = catalog.options.find((option) => option.id === kind);
  const cwd =
    (accepted ? receipt.session?.cwd : undefined) || directory || draft?.cwd ||
    parent?.cwd ||
    projects?.find((p) => p.id === parent?.projectId)?.rootPath;
  const displayedWorktree = accepted
    ? receipt.resolvedPlanExecute?.worktreeMode ?? receipt.request.planExecute?.worktreeMode
      ?? (receipt.request.planExecute ? (receipt.request.worktree ? "shared" : "none") : receipt.request.worktree !== false ? "each" : "none")
    : worktree;
  const uncertain = receipt?.state === "uncertain" || receipt?.state === "dispatching";
  const canLaunch = accepted ? !submitting && !uncertain && receipt.state !== "pending" :
    Boolean(prompt.trim()) &&
    (!menu || (!!preparation && Boolean(directory.trim()))) &&
    catalog.state === "ready" &&
    (req.planExecute ? !!workflow : selectionReady && !!spec) &&
    !taskImages.busy &&
    (!taskImages.attachments.length || !!req.planExecute || !!spec?.acceptsTask) &&
    !submitting;
  const launch = async () => {
    if (!canLaunch) return;
    setSubmitting(true);
    setError(null);
    try {
      if (accepted) { await confirmSpawn(receipt.request); return; }
      if (req.planExecute && workflow) {
        for (const role of (workflow.reviewEnabled ? ["plan", "exec", "review"] : ["plan", "exec"]) as ("plan" | "exec" | "review")[]) {
          const choice = workflow[role]!;
          try { await applyLaunchArgs(choice.agent!, null, choice.model, choice.effort); }
          catch (cause) { throw new Error(`${t(role === "review" ? "launch.reviewTitle" : role === "plan" ? "launch.planTitle" : "launch.execTitle")}: ${launchErrorText(cause)}`); }
        }
      }
      const images = taskImages.attachments.map(({ mimeType, data }) => ({ mimeType, data }));
      const imageFields = images.length || req.images ? { images } : {};
      if (menu && workflow) {
        const result = await createPlanExecute({requestId: menu.requestId, context: menu.context,
          prompt, cwd: directory.trim() || null, worktree: worktree !== "none", config: { ...workflow, worktreeMode: worktree }, ...imageFields});
        await useTermStore.getState().loadTree();
        if (result.run.state === "blocked") throw new Error(result.run.summary);
        navigatePlanExecute(planExecuteUrl(null));
        useTermStore.getState().openSession(result.planner.id);
        return;
      }
      if (req.planExecute && workflow) {
        await confirmSpawn({ ...req, prompt, worktree: worktree !== "none", planExecute: { ...workflow, worktreeMode: worktree }, ...imageFields });
        return;
      }
      await applyLaunchArgs(
        kind,
        null,
        spec?.acceptsTask ? model.trim() : null,
        spec?.effortFlag ? effort.trim() : null,
      );
      await confirmSpawn({
        ...req,
        ...imageFields,
        prompt,
        kind: kind as SpawnRequest["kind"],
        worktree: worktree !== "none",
        model: spec?.acceptsTask ? model.trim() : null,
        effort: spec?.effortFlag ? effort.trim() : null,
      });
    } catch (cause) {
      setError(launchErrorText(cause));
      setSubmitting(false);
    }
  };
  // Closing always leaves the screen. Pending and failed requests are also cancelled; a request whose
  // delivery is in progress or uncertain stays recorded and returns on the next sync.
  const close = () => {
    if (menu) { navigatePlanExecute(planExecuteUrl(null)); return; }
    const id = req.requestId;
    if (!id) { useTermStore.setState(s => ({ pendingSpawns: s.pendingSpawns.filter(item => item !== req) })); return; }
    if (!accepted || receipt.state === "failed") void cancelSpawn(id).catch(() => {});
    const url = new URL(window.location.href); url.searchParams.delete("spawnRequest");
    setInspectedRequests(previous => new Set(previous).add(id));
    window.history.pushState(window.history.state, "", url); setRouteId(null);
  };
  return (
    <section
      className="launch-dialog launch-single"
      role="dialog"
      aria-labelledby={titleId}
      onKeyDown={(e) => {
        if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
          e.preventDefault();
          void launch();
        }
      }}
    >
      <header className="launch-header">
        <div>
          <h2 id={titleId}>{t(menu ? "tree.newPlanExecuteSession" : "spawn.title")}</h2>
          <p>{t(req.planExecute ? (accepted && req.planExecute.reviewEnabled == null && receipt?.resolvedPlanExecute?.reviewEnabled == null ? "launch.legacyPlanExecuteIntro" : "launch.planExecuteIntro") : "launch.singleIntro")}</p>
        </div>
        <div className="launch-header-actions">
          {!menu && queue.length > 1 && (
            <span className="launch-badge">
              {t("spawn.remaining", queue.length - 1)}
            </span>
          )}
          <LaunchCloseButton onClose={close} />
        </div>
      </header>
      <div className="launch-body">
        <div className="launch-context">
          <span>{t(menu ? "launch.createIn" : "spawn.fromSession")}</span>
          <strong>{menu ? preparation?.locationNames.join(" / ") : (parent?.name || t("common.session"))}</strong>
          {!menu && cwd && <code title={cwd}>{cwd}</code>}
        </div>
        {accepted && <p className="launch-notice">{t("spawn.confirmedChoices")}</p>}
        {uncertain && <p className="launch-notice" role="alert">{t("spawn.deliveryUncertain")}</p>}
        {menu && !preparation && <LaunchLoadState state={preparationError ? "error" : "loading"} error={preparationError} retry={() => setPreparationRevision(r => r + 1)} />}
        {!req.planExecute && spec?.acceptsTask === false ? (
          <p className="launch-notice">{t("launch.terminalHint")}</p>
        ) : (
          <LaunchField
            label={t("spawn.promptLabel")}
            hint={t(menu ? "launch.planExecuteTaskHint" : "launch.taskHint")}
          >
            <textarea
              className="vlx-input launch-task"
              rows={6}
              autoFocus={!!menu}
              onPaste={submitting || accepted ? undefined : taskImages.onPaste}
              disabled={submitting || accepted}
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
            />
          </LaunchField>
        )}
        {taskImages.attachments.length > 0 && <div className="launch-images">
          {taskImages.attachments.map(image => <div className="launch-image" key={image.id} title={image.name}>
            <img src={dataUrl(image)} alt={image.name} />
            <button type="button" className="launch-image-remove" title={t("chat.attach.remove")} disabled={submitting || accepted || taskImages.busy} onClick={() => taskImages.remove(image.id)}>
              <Icons.close size={12} />
            </button>
          </div>)}
        </div>}
        {taskImages.busy && <p className="launch-hint" role="status">{t("common.loading")}</p>}
        {taskImages.note && <p className="launch-error" role="alert">{taskImages.note}</p>}
        {req.planExecute ? <>
          <LaunchLoadState state={catalog.state} error={catalog.error} retry={catalog.retry} />
          {catalog.state === "ready" && (!menu || preparation) && <PlanExecuteFields key={`${req.requestId}:${accepted}`} parentSessionId={req.parentSessionId}
            config={accepted ? receipt.resolvedPlanExecute ?? receipt.request.planExecute! : preparation && menu ? preparation.config : draft!.planExecute!}
            disabled={submitting} confirmed={accepted} resolved={!!menu} cwd={cwd} options={catalog.options} onChange={setWorkflow} />}
        </> : <section className="launch-panel">
          <h3>{t("launch.runtime")}</h3>
          <LaunchLoadState state={catalog.state} error={catalog.error} retry={catalog.retry} />
          {catalog.state === "ready" && (
            <>
              <LaunchField label={t("spawn.agentLabel")}>
                <AgentSelect
                  key={String(submitting || accepted)}
                  value={kind as SessionKind | ""}
                  disabled={submitting || accepted}
                  onChange={(value) => { if (!accepted && value) setKind(value); }}
                  width="100%"
                  ariaLabel={t("spawn.agentLabel")}
                  options={catalog.options.map((o) => ({
                    id: o.id as SessionKind,
                    label: o.id === "terminal" ? t("kind.terminal") : o.label,
                  }))}
                />
              </LaunchField>
              {!selectionReady && (
                <LaunchLoadState
                  state={selectionError || !req.requestId ? "error" : "loading"}
                  error={!req.requestId ? t("spawn.requestUnavailable") : selectionError}
                  retry={() => setRevision((r) => r + 1)}
                />
              )}
              {selectionReady && spec?.acceptsTask && (
                <div className="launch-model-grid">
                  <ModelEffortFields spec={spec} model={model} effort={effort} disabled={submitting || accepted}
                    context={{ parentSessionId: req.parentSessionId, cwd, inheritArgs: true }}
                    onChange={(nextModel, nextEffort) => { if (!accepted) { setModel(nextModel); setEffort(nextEffort); } }} />
                </div>
              )}
            </>
          )}
        </section>}
        {menu && <LaunchField label={t("launch.workingDirectory")}>
          <input className="vlx-input" value={directory} disabled={!preparation} placeholder="/path/to/project" onChange={e => setDirectory(e.target.value)} />
        </LaunchField>}
        <WorktreeChoices
          single={!req.planExecute}
          disabled={submitting || accepted}
          workflow={!!req.planExecute}
          value={displayedWorktree}
          onChange={(value) => { if (!accepted) setWorktree(value); }}
        />
        {!uncertain && (error || receipt?.error) && (
          <p ref={errorElement} className="launch-error" role="alert">
            {t("launch.startError")} <span className="launch-error-detail">{error || receipt?.error}</span>
          </p>
        )}
      </div>
      <footer className="launch-footer">
        <span className="launch-hint">{t(req.planExecute ? "launch.planExecuteResult" : "launch.singleResult")}</span>
        <div className="launch-actions">
          {accepted && receipt.session && <button type="button" className="vlx-btn" onClick={() => {
            // Keep mobile navigation and card dismissal in one history entry.
            const mobile = isMobileView();
            const url = new URL(mobile ? mobileSessionUrl(receipt.session!.id) : window.location.href, window.location.href);
            url.searchParams.delete("spawnRequest");
            setInspectedRequests(previous => new Set(previous).add(receipt.requestId));
            window.history.pushState(window.history.state, "", url); setRouteId(null);
            if (mobile) window.dispatchEvent(new PopStateEvent("popstate"));
            useTermStore.getState().openSession(receipt.session!.id);
          }}>{t("common.open")}</button>}
          <button
            type="button"
            className="vlx-btn"
            onClick={() => {
              if (menu) { navigatePlanExecute(planExecuteUrl(null)); return; }
              setSubmitting(true);
              void cancelSpawn(req.requestId).catch((cause) => setError(launchErrorText(cause)))
                .finally(() => setSubmitting(false));
            }}
            disabled={submitting || (accepted && receipt.state !== "failed")}
          >
            {t("common.cancel")}
          </button>
          <button
            type="button"
            className="vlx-btn vlx-btn-primary"
            disabled={!canLaunch}
            onClick={() => void launch()}
          >
            {t(submitting ? "launch.starting" : accepted ? "common.retry" : menu ? "launch.createAndStart" : "spawn.launch")}
          </button>
        </div>
      </footer>
    </section>
  );
}
