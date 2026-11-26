//! Review backend-persisted tasks before launching their execution conversations.
import { useEffect, useId, useRef, useState } from "react";
import { useT } from "../i18n";
import { cancelSplitProposal, confirmSplitProposal, pendingSplitProposals, readSplitProposal, type SplitProposal, type SplitTask } from "../ipc/launch";
import { isTauri, listen } from "../ipc/transport";
import { wsClient } from "../ipc/wsClient";
import { useTermStore } from "../store/termStore";
import { useSuspendNativeViews } from "../hooks/nativeViewSuspend";
import type { SessionKind } from "../types";
import AgentSelect from "./AgentSelect";
import { Backdrop } from "./Backdrop";
import { LaunchCloseButton, LaunchField, LaunchLoadState, ModelEffortFields, launchErrorText, useLaunchOptions } from "./LaunchFields";
import { navigatePlanExecute, usePlanExecuteMenuRoute } from "./planExecuteNavigation";

export function splitReviewUrl(runId: string | null, taskId?: string) {
  const url = new URL(window.location.href);
  url.searchParams.delete("splitReview"); url.searchParams.delete("splitTask");
  if (runId) { url.searchParams.set("splitReview", runId); if (taskId) url.searchParams.set("splitTask", taskId); }
  return url.href;
}
function readRoute() { const p = new URLSearchParams(window.location.search); return { runId: p.get("splitReview"), taskId: p.get("splitTask") }; }
type Draft = SplitTask & { draftId: string };

export function SplitTaskConfirmModal() {
  const t = useT();
  const launchRoute = usePlanExecuteMenuRoute();
  const hasSpawnReview = useTermStore(state => state.pendingSpawns.length > 0);
  const titleId = useId();
  const dialog = useRef<HTMLDivElement>(null);
  const [route, setRoute] = useState(readRoute);
  const [pending, setPending] = useState<string[]>([]);
  const [pendingError, setPendingError] = useState<string | null>(null);
  const [proposal, setProposal] = useState<SplitProposal | null>(null);
  const [drafts, setDrafts] = useState<Draft[]>([]);
  const [removed, setRemoved] = useState<{ task: Draft; index: number } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [revision, setRevision] = useState(0);
  const catalog = useLaunchOptions(Boolean(route.runId));
  useSuspendNativeViews(Boolean(route.runId));
  const refreshPending = async () => {
    try { setPending(await pendingSplitProposals()); setPendingError(null); }
    catch (cause) { setPendingError(launchErrorText(cause)); }
  };
  useEffect(() => {
    let live = true;
    const refresh = async (open = false) => {
      try {
        const ids = await pendingSplitProposals();
        if (!live) return;
        setPending(ids); setPendingError(null);
        // Keep the current launch draft in front; the pending link becomes available after it closes.
        const params = new URL(window.location.href).searchParams;
        const launchOpen = params.has("planExecute") || params.has("bindPlanExecute") || useTermStore.getState().pendingSpawns.length > 0;
        if (open && !launchOpen && !readRoute().runId && ids.length) navigatePlanExecute(splitReviewUrl(ids[0]));
      } catch (cause) { if (live) setPendingError(launchErrorText(cause)); }
    };
    void refresh();
    const unlisten = listen<{ runId: string; resolved?: boolean }>("plan-execute://proposal", event => {
      void refresh(!event.resolved);
      // Another client may confirm or cancel the proposal currently being edited.
      if (event.resolved && readRoute().runId === event.runId) setRevision(r => r + 1);
    });
    const off = isTauri ? undefined : wsClient.onConnState(state => { if (state === "online") { void refresh(); setRevision(r => r + 1); } });
    const update = () => setRoute(readRoute());
    window.addEventListener("popstate", update);
    return () => { live = false; void unlisten.then(fn => fn()); off?.(); window.removeEventListener("popstate", update); };
  }, []);
  useEffect(() => {
    if (!route.runId) { setProposal(null); return; }
    let live = true; setLoading(true);
    void readSplitProposal(route.runId).then(value => {
      if (!live) return;
      setProposal(value); setDrafts(value.tasks.map((task, index) => ({ ...task, draftId: String(index) })));
      setRemoved(null);
    }).catch(cause => { if (live) { setProposal(null); setError(launchErrorText(cause)); } }).finally(() => { if (live) setLoading(false); });
    return () => { live = false; };
  }, [route.runId, revision]);
  useEffect(() => {
    if (!route.runId) return;
    const previous = document.activeElement as HTMLElement | null;
    dialog.current?.focus();
    return () => { if (previous?.isConnected) previous.focus(); };
  }, [route.runId]);
  useEffect(() => { setError(null); }, [route.runId]);
  const close = () => navigatePlanExecute(splitReviewUrl(null));
  const current = drafts.find(draft => draft.draftId === route.taskId) ?? drafts[0];
  const options = catalog.options.filter(option => option.supportsPlanExecute);
  const spec = options.find(option => option.id === current?.config.agent);
  const editable = proposal?.state === "pending" && proposal.run.state === "awaiting_confirmation";
  const canConfirm = editable && !loading && !busy && catalog.state === "ready" && drafts.length > 0 &&
    drafts.every(draft => draft.name.trim() && draft.prompt.trim() && options.some(option => option.id === draft.config.agent));
  const patch = (value: Partial<SplitTask>) => setDrafts(list => list.map(draft => draft.draftId === current?.draftId ? { ...draft, ...value } : draft));
  const run = async (cancel: boolean) => {
    if (!proposal || busy) return;
    setBusy(true); setError(null);
    try {
      if (cancel) await cancelSplitProposal(proposal.runId, proposal.proposalId);
      else {
        const result = await confirmSplitProposal({ runId: proposal.runId, proposalId: proposal.proposalId,
          tasks: drafts.map(({ name, prompt, config }) => ({ name, prompt, config })) });
        await useTermStore.getState().loadTree();
        if (result.errors.length) {
          setError(result.errors.map(item => item.error).join("\n")); setRevision(r => r + 1);
          return;
        }
        useTermStore.getState().openSession(proposal.plannerId);
      }
      await refreshPending(); close();
    } catch (cause) { setError(launchErrorText(cause)); }
    finally { setBusy(false); }
  };
  if (!route.runId) return !launchRoute && !hasSpawnReview && (pending.length || pendingError) ? <aside className="split-pending launch-panel" aria-label={t("launch.splitReview")}>
    {pending.map((id, index) => <a key={id} href={splitReviewUrl(id)} onClick={event => { if (!event.ctrlKey && !event.metaKey && !event.shiftKey && event.button === 0) { event.preventDefault(); navigatePlanExecute(event.currentTarget.href); } }}>{t("launch.splitReview")} · {index + 1}</a>)}
    {pendingError && <div role="alert">{pendingError} <button className="vlx-btn" onClick={() => void refreshPending()}>{t("common.retry")}</button></div>}
  </aside> : null;
  return <Backdrop onClose={() => {}}><div ref={dialog} tabIndex={-1} className="launch-dialog launch-batch" role="dialog" aria-modal="true" aria-labelledby={titleId}
    onKeyDown={event => {
      if (event.key !== "Tab") return;
      const nodes = Array.from(event.currentTarget.querySelectorAll<HTMLElement>('button:not(:disabled), a[href], input:not(:disabled), textarea:not(:disabled), [tabindex="0"]')).filter(node => node.getClientRects().length);
      if (!nodes.length) return;
      if (event.shiftKey && (document.activeElement === nodes[0] || document.activeElement === dialog.current)) { event.preventDefault(); nodes.at(-1)?.focus(); }
      else if (!event.shiftKey && document.activeElement === nodes.at(-1)) { event.preventDefault(); nodes[0].focus(); }
    }}>
    <header className="launch-header"><div><h2 id={titleId}>{t("launch.splitReview")}</h2><p>{t("launch.splitReviewHint")}</p></div>
      <LaunchCloseButton onClose={close} /></header>
    <div className="launch-body">
      {loading && <p role="status">{t("common.loading")}</p>}
      {error && <div className="launch-notice launch-error" role="alert">{error}<button className="vlx-btn" disabled={busy} onClick={() => setRevision(r => r + 1)}>{t("common.retry")}</button></div>}
      {proposal && !loading && <>
        <section className="launch-panel"><div className="launch-context"><span>{t("launch.planTitle")}</span><strong>{proposal.plannerName}</strong><code>{proposal.cwd}</code></div><p className="split-original-task">{proposal.task}</p></section>
        {editable ? <>
          <LaunchLoadState state={catalog.state} error={catalog.error} retry={catalog.retry} />
          <div className="split-review-grid">
            <nav className="launch-panel split-task-list" aria-label={t("launch.splitTasks")}>
              {drafts.map((draft, index) => <a key={draft.draftId} aria-current={draft === current ? "page" : undefined} href={splitReviewUrl(proposal.runId, draft.draftId)}
                onClick={event => { if (!event.ctrlKey && !event.metaKey && !event.shiftKey && event.button === 0) { event.preventDefault(); navigatePlanExecute(event.currentTarget.href); } }}>{index + 1}. {draft.name}</a>)}
            </nav>
            {current && <section className="launch-panel">
              <LaunchField label={t("orch.nameLabel")}><input className="vlx-input" value={current.name} disabled={busy} maxLength={80} onChange={event => patch({ name: event.target.value })} /></LaunchField>
              <LaunchField label={t("orch.promptLabel")}><textarea className="vlx-input launch-task" value={current.prompt} disabled={busy} onChange={event => patch({ prompt: event.target.value })} /></LaunchField>
              <LaunchField label={t("spawn.agentLabel")}>
                <AgentSelect
                  value={(current.config.agent ?? "") as SessionKind | ""}
                  disabled={busy}
                  width="100%"
                  ariaLabel={t("spawn.agentLabel")}
                  options={options.map(option => ({ id: option.id as SessionKind, label: option.label }))}
                  onChange={agent => { if (!busy && agent) patch({ config: { agent: agent as SplitTask["config"]["agent"], model: "", effort: "" } }); }}
                />
              </LaunchField>
              {spec && <div className="launch-model-grid"><ModelEffortFields spec={spec} model={current.config.model ?? ""} effort={current.config.effort ?? ""}
                context={{ parentSessionId: proposal.plannerId, cwd: proposal.cwd, inheritArgs: false }}
                onChange={(model, effort) => { if (!busy) patch({ config: { ...current.config, model, effort } }); }} /></div>}
              <div className="launch-inline"><button className="vlx-btn" disabled={busy || drafts.length <= 1} onClick={() => { setRemoved({ task: current, index: drafts.indexOf(current) }); setDrafts(list => list.filter(draft => draft !== current)); }}>{t("orch.remove")}</button>
                {removed && <button className="vlx-btn" disabled={busy} onClick={() => { setDrafts(list => { const next = [...list]; next.splice(removed.index, 0, removed.task); return next; }); setRemoved(null); }}>{t("launch.undoRemove")}</button>}</div>
            </section>}
          </div>
        </> : <section className="launch-panel"><p>{t(proposal.state === "confirmed" ? "launch.splitConfirmed" : "launch.splitClosed")}</p>
          {proposal.executionTasks.map(task => <div key={task.run.id}><strong>{task.name}</strong><code> {task.run.id}</code></div>)}
          {proposal.state === "confirmed" && !["completed", "stopped"].includes(proposal.run.state) && <button className="vlx-btn" disabled={busy} onClick={() => void run(false)}>{t("launch.splitRetry")}</button>}
        </section>}
      </>}
    </div>
    <footer className="launch-footer"><span className="launch-hint">{t(proposal?.run.config?.worktreeMode === "each" ? "launch.workflowDirectoryEachHint" : "launch.splitSharedDirectory")}</span><div className="launch-actions">
      {editable && <><button type="button" className="vlx-btn" disabled={busy} onClick={() => void run(true)}>{t("common.cancel")}</button>
        <button type="button" className="vlx-btn vlx-btn-primary" disabled={!canConfirm} onClick={() => void run(false)}>{busy ? t("common.loading") : t("orch.launch", drafts.length)}</button></>}
    </div></footer>
  </div></Backdrop>;
}
