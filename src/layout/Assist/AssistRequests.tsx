//! Actions a session's agent submitted with `vapp`, shown in that session's view between the conversation
//! and the input, like a permission request: the agent waits until the user confirms or skips each one.
//! Consecutive host actions that need no input are confirmed together on one card, so reorganizing a
//! project's sessions takes one click rather than one per change.

import { useEffect, useState, useSyncExternalStore } from "react";
import Icons from "../../components/Icons";
import Select from "../../components/Select";
import { useT } from "../../i18n";
import { assistClaim, assistGet, assistMark, assistPending, assistRun, assistRunAll, assistSkipAll, type AssistAction, type AssistPlan } from "../../ipc/assist";
import { onAssistChanged, onAssistRequest } from "../../ipc/events";
import { useTermStore } from "../../store/termStore";
import { afterHostAction, describeAction, label, performAction, references, waitsForEarlier } from "./actions";
import "./assist.css";

/** Bumped for each new request so every session lane reads its list again. */
let revision = 0;
const listeners = new Set<() => void>();
const subscribe = (notify: () => void) => { listeners.add(notify); return () => { listeners.delete(notify); }; };

const refresh = () => { revision += 1; listeners.forEach(notify => notify()); };

/** Runs client-owned actions of a `noConfirm` plan in order, claiming each one so only one client executes it. */
async function runNoConfirmWindowActions(id: string): Promise<AssistPlan | null> {
  const initial = await assistGet(id).catch(() => null);
  if (!initial || !initial.noConfirm) return initial;
  let plan: AssistPlan = initial;
  for (let index = 0; index < plan.actions.length; index += 1) {
    const action = plan.actions[index];
    if (action.backend || action.status !== "pending" || waitsForEarlier(action)) continue;
    const claimed = await assistClaim(id, index).catch(() => null);
    if (!claimed) continue;
    plan = claimed.plan;
    if (!claimed.claimed) continue;
    try {
      await performAction(plan.actions[index]);
      plan = await assistMark(id, index, "done");
    } catch (e) {
      plan = await assistMark(id, index, "failed", message(e)).catch(() => plan);
    }
  }
  return plan;
}

/** Registered once at the app root: a new request refreshes the lanes and brings its session into view;
 *  `noConfirm` plans run their window actions directly without stealing focus unless user choice remains;
 *  decisions made in another client refresh them too. */
export function AssistRequestListener() {
  useEffect(() => {
    const stops: (() => void)[] = []; let live = true;
    const keep = (stop: () => void) => { if (live) stops.push(stop); else stop(); };
    const focusSession = (sessionId: string) => {
      const state = useTermStore.getState();
      if (state.activeSessionId !== sessionId && state.sessions.some(session => session.id === sessionId)) state.openSession(sessionId);
    };
    void onAssistRequest(({ id, sessionId, noConfirm }) => {
      if (noConfirm) {
        void runNoConfirmWindowActions(id).then((plan) => {
          if (!live) return;
          refresh();
          if (plan?.actions.some(action => action.status === "pending")) focusSession(sessionId);
        });
        return;
      }
      refresh();
      focusSession(sessionId);
    }).then(keep);
    void onAssistChanged((change) => {
      if (change.knowledgeChanged) window.dispatchEvent(new Event("notebook:vaultsChanged"));
      if (change.killedSessions?.length) {
        const state = useTermStore.getState();
        for (const sid of change.killedSessions) {
          state.setRuntime(sid, { status: "exited", pid: undefined });
          state.closeSession(sid);
        }
      }
      refresh();
    }).then(keep);
    return () => { live = false; stops.forEach(stop => stop()); };
  }, []);
  return null;
}

const open = (action: { status: string }) => action.status === "pending" || action.status === "running";
const message = (error: unknown) => String(error).replace(/^Error: /, "");
/** An import that found several folders waits for the user to pick one. */
const needsChoice = (action: AssistAction) => Array.isArray(action.candidates) && action.candidates.length > 1;
const batchable = (action: AssistAction) => action.backend && !needsChoice(action);

/** The open actions in order, with each run of consecutive batchable actions gathered into one group. */
export function cardGroups(plan: AssistPlan): number[][] {
  const groups: number[][] = [];
  plan.actions.forEach((action, index) => {
    if (!open(action)) return;
    const last = groups[groups.length - 1];
    if (last && batchable(action) && batchable(plan.actions[last[last.length - 1]])) last.push(index);
    else groups.push([index]);
  });
  return groups;
}

/** This session's waiting requests; hidden when there are none. */
export function AssistRequests({ sessionId, className }: { sessionId: string; className: string }) {
  const version = useSyncExternalStore(subscribe, () => revision);
  const [plans, setPlans] = useState<AssistPlan[]>([]);
  // Requests decided here with failed actions stay until the user has read which ones failed.
  const [reports, setReports] = useState<AssistPlan[]>([]);
  useEffect(() => {
    let alive = true;
    // An older host without this command lists nothing; the lane simply stays hidden.
    assistPending(sessionId).then(list => { if (alive) setPlans(Array.isArray(list) ? list : []); }, () => {});
    return () => { alive = false; };
  }, [sessionId, version]);
  // A request leaves the lane once the user has decided on every action; the agent reports the outcome.
  const waiting = plans.filter(plan => plan.actions.some(open));
  const shown = [...waiting, ...reports.filter(report => !waiting.some(plan => plan.id === report.id))];
  if (shown.length === 0) return null;
  const update = (next: AssistPlan) => {
    setPlans(list => list.map(plan => plan.id === next.id ? next : plan));
    if (!next.actions.some(open) && next.actions.some(action => action.status === "failed")) setReports(list => [...list.filter(p => p.id !== next.id), next]);
  };
  const close = (id: string) => setReports(list => list.filter(plan => plan.id !== id));
  return <div className={className}>{shown.map(plan => <RequestView key={plan.id} plan={plan} onPlan={update} onClose={() => close(plan.id)} />)}</div>;
}

function RequestView({ plan, onPlan, onClose }: { plan: AssistPlan; onPlan: (plan: AssistPlan) => void; onClose: () => void }) {
  const t = useT();
  const summary = plan.summary?.trim();
  const groups = cardGroups(plan);
  const failed = plan.actions.filter(action => action.status === "failed");
  return <section className="assist-request-group" aria-label={t("assist.confirmHint")}>
    <header><Icons.sparkle size={13} /><strong>{summary || t("assist.confirmHint")}</strong></header>
    {summary && groups.length > 0 && <p className="assist-note">{t("assist.confirmHint")}</p>}
    {groups.length > 0 && <ol className="assist-cards">{groups.map(indices => indices.length === 1
      ? <ActionCardView key={`${plan.id}:${indices[0]}`} plan={plan} index={indices[0]} onPlan={onPlan} />
      : <BatchCardView key={`${plan.id}:${indices.join(",")}`} plan={plan} indices={indices} onPlan={onPlan} />)}</ol>}
    {failed.length > 0 && <div className="assist-failed" role="status"><p>{t("assist.failed", failed.length)}</p>
      <ul>{failed.map((action, i) => <li key={i}>{describeAction(action, t).title}</li>)}</ul>
      {groups.length === 0 && <div className="assist-card-actions"><button type="button" className="vlx-btn" onClick={onClose}>{t("common.close")}</button></div>}</div>}
  </section>;
}

/** True when an action in the group needs an item that an action outside the group has yet to create. */
function waitsOutside(plan: AssistPlan, indices: number[]) {
  const created = new Set(indices.map(i => plan.actions[i].ref).filter(Boolean));
  return indices.some(i => references(plan.actions[i]).some(name => !created.has(name)));
}

/** Several host changes in a row, applied or skipped together in the order the agent gave. */
function BatchCardView({ plan, indices, onPlan }: { plan: AssistPlan; indices: number[]; onPlan: (plan: AssistPlan) => void }) {
  const t = useT();
  const [busy, setBusy] = useState(false); const [error, setError] = useState("");
  const actions = indices.map(i => plan.actions[i]);
  const cards = actions.map(action => describeAction(action, t));
  const danger = cards.some(card => card.danger);
  const blocked = waitsOutside(plan, indices);
  const domains = new Set(actions.map(action => action.domain));
  const decide = async (work: () => Promise<AssistPlan>) => {
    setBusy(true); setError("");
    try {
      const next = await work(); onPlan(next);
      indices.forEach(i => { if (next.actions[i]?.status === "done") afterHostAction(next.actions[i]); });
    } catch (e) {
      const next = await assistGet(plan.id).catch(() => null);
      if (next) onPlan(next);
      setError(label(t, `assist.reason.${message(e)}`, t("assist.error")));
    } finally { setBusy(false); }
  };
  const running = busy || actions.some(action => action.status === "running");
  return <li className={`assist-card assist-batch${danger ? " danger" : ""}`}>
    <div className="assist-card-head">{domains.size === 1 ? <DomainIcon domain={actions[0].domain} /> : <Icons.sparkle size={13} />}<strong>{t("assist.batchTitle", indices.length)}</strong></div>
    <ol className="assist-batch-list">{cards.map((card, i) => <li key={indices[i]} className={card.danger ? "danger" : undefined}>
      {card.title}{card.details.length > 0 && <span className="assist-batch-detail">{card.details.join(" · ")}</span>}</li>)}</ol>
    {blocked && <p className="assist-note">{t("assist.waitsForEarlier")}</p>}
    {error && <p className="assist-error" role="alert">{error}</p>}
    <div className="assist-card-actions">
      <button type="button" className="vlx-btn" disabled={running} onClick={() => void decide(() => assistSkipAll(plan.id, indices))}>{t("assist.skipAll")}</button>
      <button type="button" className="vlx-btn vlx-btn-primary assist-confirm" disabled={running || blocked} onClick={() => void decide(() => assistRunAll(plan.id, indices))}>
        <Icons.check size={12} />{t("assist.applyAll", indices.length)}</button>
    </div>
  </li>;
}

function ActionCardView({ plan, index, onPlan }: { plan: AssistPlan; index: number; onPlan: (plan: AssistPlan) => void }) {
  const t = useT(); const action = plan.actions[index]; const card = describeAction(action, t);
  const [busy, setBusy] = useState(false); const [error, setError] = useState("");
  // A single folder found needs no choice; the card names it instead.
  const candidates = needsChoice(action) ? action.candidates as string[] : [];
  const [choice, setChoice] = useState(candidates[0] ?? "");
  const blocked = waitsForEarlier(action);
  const confirm = async () => {
    setBusy(true); setError("");
    try {
      if (action.backend) { const next = await assistRun(plan.id, index, candidates.length > 0 ? choice : undefined); onPlan(next); afterHostAction(next.actions[index]); }
      else { await performAction(action); onPlan(await assistMark(plan.id, index, "done")); }
    } catch (e) {
      const reason = message(e);
      // A host action records its own failure; a window action reports it here.
      const next = action.backend ? await assistGet(plan.id).catch(() => null) : await assistMark(plan.id, index, "failed", reason).catch(() => null);
      if (next) onPlan(next);
      // Host messages stay with the agent; the card shows a known reason or a plain message.
      if (!next?.actions[index]?.error) setError(label(t, `assist.reason.${reason}`, t("assist.error")));
    } finally { setBusy(false); }
  };
  const dismiss = async () => {
    setBusy(true); setError("");
    try { onPlan(await assistMark(plan.id, index, "dismissed")); } catch (e) { setError(message(e)); } finally { setBusy(false); }
  };
  const running = busy || action.status === "running";
  return <li className={`assist-card${card.danger ? " danger" : ""}`}>
    <div className="assist-card-head"><DomainIcon domain={action.domain} /><strong>{card.title}</strong></div>
    {card.details.length > 0 && <ul className="assist-details">{card.details.map((line, i) => <li key={i}>{line}</li>)}</ul>}
    {candidates.length > 0 && <div className="assist-choices"><span>{t("assist.chooseFolder")}</span>
      <Select width="100%" mono menuPortal value={choice} ariaLabel={t("assist.chooseFolder")} onChange={setChoice} options={candidates.map(path => ({ value: path, label: path }))} /></div>}
    {card.opensForm && <p className="assist-note">{t("assist.opensForm")}</p>}
    {blocked && <p className="assist-note">{t("assist.waitsForEarlier")}</p>}
    {error && <p className="assist-error" role="alert">{error}</p>}
    <div className="assist-card-actions">
      <button type="button" className="vlx-btn" disabled={running} onClick={() => void dismiss()}>{t("assist.skip")}</button>
      <button type="button" className="vlx-btn vlx-btn-primary assist-confirm" disabled={running || blocked || (candidates.length > 0 && !choice)} onClick={() => void confirm()}>
        <Icons.check size={12} />{t(card.opensForm ? "assist.openForm" : "assist.confirm")}</button>
    </div>
  </li>;
}

function DomainIcon({ domain }: { domain: AssistAction["domain"] }) {
  const size = 13;
  if (domain === "files") return <Icons.file size={size} />;
  if (domain === "knowledge") return <Icons.notebook size={size} />;
  if (domain === "settings") return <Icons.gear size={size} />;
  return <Icons.project size={size} />;
}
