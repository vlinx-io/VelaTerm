import { useEffect, useRef, useState } from "react";
import { chatZcodeStartPlan, type ZcodeStartPlan } from "../../../ipc/chat";
import { useT } from "../../../i18n";

/** 权益仅由 CLI 提供；选择器不保存凭据，也不推测套餐模型。 */
export function ZcodeStartPlanStatus({ sessionId, onRefreshed }: { sessionId: string; onRefreshed: () => void }) {
  const t = useT();
  const [attempt, setAttempt] = useState(0);
  const [state, setState] = useState<{ sessionId: string; plans: ZcodeStartPlan[]; loading: boolean; failed: boolean }>({ sessionId, plans: [], loading: true, failed: false });
  const refreshed = useRef(onRefreshed);
  refreshed.current = onRefreshed;
  useEffect(() => {
    let disposed = false;
    setState({ sessionId, plans: [], loading: true, failed: false });
    void chatZcodeStartPlan(sessionId).then(result => {
      if (disposed) return;
      if (!Array.isArray(result?.plans)) throw new Error("Invalid Start Plan response");
      setState({ sessionId, plans: result.plans, loading: false, failed: false });
      refreshed.current();
    }).catch(() => {
      if (!disposed) setState({ sessionId, plans: [], loading: false, failed: true });
    });
    return () => { disposed = true; };
  }, [sessionId, attempt]);
  const current = state.sessionId === sessionId ? state : { plans: [], loading: true, failed: false };
  const date = (seconds: number) => new Date(seconds * 1000).toLocaleString();
  return <section className="sv-model-catalog-status" aria-label="Start Plan" style={{ display: "block", maxHeight: 240, overflowY: "auto" }}>
    <div style={{ display: "flex", justifyContent: "space-between", gap: 8 }}>
      <strong>Start Plan</strong>
      <button type="button" className="sv-model-catalog-refresh" disabled={current.loading} onClick={() => setAttempt(value => value + 1)}>{t("common.refresh")}</button>
    </div>
    {current.loading && <div role="status">{t("common.loading")}</div>}
    {current.failed && <div role="status">{t("chat.zcode.planLoadFailed")}</div>}
    {!current.loading && !current.failed && current.plans.length === 0 && <div>{t("chat.zcode.planUnsupported")}</div>}
    {current.plans.map(plan => <div key={plan.providerId} style={{ marginTop: 8 }}>
      <div>{plan.label} · {t(`chat.zcode.plan.${plan.status}`)}</div>
      {plan.effectiveAt !== undefined && <div>{date(plan.effectiveAt)}</div>}
      {plan.message && <div>{plan.message}</div>}
      {plan.plans.map((item, index) => <div key={index} style={{ marginTop: 4 }}>
        <div>{item.name}</div>
        {item.effectiveAt !== undefined && <div>{t("chat.zcode.planEffective")} {date(item.effectiveAt)}</div>}
        {item.endsAt !== undefined && <div>{t("chat.zcode.planEnds")} {date(item.endsAt)}</div>}
      </div>)}
      {plan.balances.map((balance, index) => <div key={index}>
        {balance.name} · {t("chat.zcode.planRemaining")} {balance.available ?? "—"} / {balance.total ?? "—"} {balance.unit}
        {balance.used !== undefined && <> · {t("chat.zcode.planUsed")} {balance.used}</>}
      </div>)}
    </div>)}
  </section>;
}
