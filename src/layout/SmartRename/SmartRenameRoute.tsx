import { useEffect, useRef, useState } from "react";
import AgentSelect from "../../components/AgentSelect";
import { FieldStack } from "../../components/Field";
import { FormModal } from "../../components/FormModal";
import { LaunchLoadState, ModelEffortFields } from "../../components/LaunchFields";
import { useT, type I18nKey } from "../../i18n";
import { cancelSessionTitle, renameSessionWithAgent, sessionTitleOptions } from "../../ipc/tree";
import { genId } from "../../genId";
import { useTermStore } from "../../store/termStore";
import type { SessionKind } from "../../types";
import { navigateSmartRename, readSmartRenameRoute, useSmartRenameRoute, writeSmartRenameSelection, type SmartRenameRoute as Route } from "./navigation";

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
  return route ? <AgentChoice route={route} key={route.sessionId} /> : null;
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
