//! Reusable form fields for reviewing child-session launch requests.

import { useEffect, useId, useState, type ReactNode } from "react";
import { useT } from "../i18n";
import { launchModels, launchOptions, type LaunchOption, type LaunchModelContext, type LaunchModelCatalog } from "../ipc/launch";
import type { WorktreeMode } from "../ipc/events";
import Combo from "./Combo";
import { Field } from "./Field";
import { Icons } from "./Icons";
import "./launch-dialog.css";

export function launchErrorText(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

/** Header close control shared by launch dialogs. It is never disabled, so no error or pending work can trap the user. */
export function LaunchCloseButton({ onClose }: { onClose: () => void }) {
  const t = useT();
  return (
    <button type="button" className="icon-btn launch-close" aria-label={t("common.close")} title={t("common.close")} onClick={onClose}>
      <Icons.close size={16} aria-hidden="true" />
    </button>
  );
}

export function useLaunchOptions(active: boolean) {
  const [options, setOptions] = useState<LaunchOption[]>([]);
  const [state, setState] = useState<"loading" | "ready" | "error">("loading");
  const [revision, setRevision] = useState(0);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!active) return;
    let live = true;
    setState("loading"); setError(null);
    void launchOptions()
      .then((value) => {
        if (!live) return;
        setOptions(value);
        setState(value.length ? "ready" : "error");
      })
      .catch((cause) => {
        if (live) { setState("error"); setError(launchErrorText(cause)); }
      });
    return () => {
      live = false;
    };
  }, [active, revision]);
  return { options, state, error, retry: () => setRevision((r) => r + 1) };
}

export function LaunchField({
  label,
  children,
  hint,
}: {
  label: string;
  children: ReactNode;
  hint?: string;
}) {
  return <Field label={label} hint={hint}>{children}</Field>;
}

const modelRequests = new Map<string, Promise<LaunchModelCatalog>>();
function models(kind: string, context: LaunchModelContext) {
  const key = JSON.stringify([kind, context]);
  const pending = modelRequests.get(key);
  if (pending) return pending;
  const request = launchModels(kind, context).finally(() => modelRequests.delete(key));
  modelRequests.set(key, request);
  return request;
}

/** The model catalogue an agent offers for a launch context. `current` is null while a request is pending;
 * otherwise it holds the catalogue or the error for the latest context. */
export function useLaunchModels(kind: string | null | undefined, context: LaunchModelContext = {}) {
  const key = kind ? JSON.stringify([kind, context.parentSessionId || null, context.cwd || null, !!context.inheritArgs]) : "";
  const [response, setResponse] = useState<{ key: string; catalog?: LaunchModelCatalog; error?: string } | null>(null);
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    let live = true;
    setResponse(null);
    if (!key) return;
    // Directory editing should not start a discovery process for every keystroke.
    const timer = setTimeout(() => {
      const [kind, parentSessionId, cwd, inheritArgs] = JSON.parse(key) as [string, string | null, string | null, boolean];
      void models(kind, { parentSessionId, cwd, inheritArgs }).then(catalog => {
        if (live) setResponse({ key, catalog });
      }).catch(cause => { if (live) setResponse({ key, error: launchErrorText(cause) }); });
    }, 180);
    return () => { live = false; clearTimeout(timer); };
  }, [key, revision]);
  const current = key && response?.key === key ? response : null;
  return { current, catalog: current?.catalog, retry: () => setRevision(r => r + 1) };
}

/** A model and its effort share one native catalogue and one atomic draft update. */
export function ModelEffortFields({
  spec, model, effort, onChange, onModelCommit, onEffortCommit,
  context = {}, modelPlaceholder, effortPlaceholder, inheritedModel, disabled = false,
}: {
  spec: LaunchOption;
  model: string;
  effort: string;
  onChange: (model: string, effort: string) => void;
  onModelCommit?: (model: string) => void;
  onEffortCommit?: (effort: string) => void;
  context?: LaunchModelContext;
  modelPlaceholder?: string;
  effortPlaceholder?: string;
  inheritedModel?: string;
  disabled?: boolean;
}) {
  const t = useT();
  const { current, catalog, retry } = useLaunchModels(spec.id, context);
  const selected = catalog?.models.find(entry => entry.id === (model || inheritedModel));
  const levels = selected?.effortLevels ?? catalog?.effortLevels ?? [];
  const defaultEffort = effortPlaceholder || t("spawn.modelDefault");
  return <>
    <div className="launch-field">
      <LaunchField label={t("spawn.modelLabel")}>
        <Combo key={String(disabled)} value={model} disabled={disabled} onChange={next => { if (!disabled) onChange(next, effort); }}
          onCommit={next => {
            if (disabled) return;
            const supported = catalog?.models.find(entry => entry.id === next)?.effortLevels;
            if (effort && supported && !supported.includes(effort)) {
              onChange(next, "");
              onEffortCommit?.("");
            }
            onModelCommit?.(next);
          }}
          allowCustom width="100%" menuPortal mono ariaLabel={t("spawn.modelLabel")}
          placeholder={modelPlaceholder || t("spawn.modelDefault")}
          options={(catalog?.models ?? []).map(entry => ({ value: entry.id, label: entry.id, hint: entry.label !== entry.id ? entry.label : undefined }))}
        />
      </LaunchField>
      {!current && <span className="launch-hint" role="status">{t("spawn.modelLoading")}</span>}
      {current && (!catalog?.models.length || current.error) && <div className="launch-hint">
        <div className="launch-inline"><span>{t("spawn.modelListUnavailable")}</span>
          <button type="button" className="launch-link" onClick={retry}>{t("common.retry")}</button>
        </div>
        {current.error && <span className="launch-error-detail">{current.error}</span>}
      </div>}
    </div>
    {spec.effortFlag && <LaunchField label={t("spawn.effortLabel")}>
      <Combo key={String(disabled)} value={effort} disabled={disabled} onChange={next => { if (!disabled) onChange(model, next); }} onCommit={next => { if (!disabled) onEffortCommit?.(next); }}
        allowCustom width="100%" menuPortal mono ariaLabel={t("spawn.effortLabel")}
        placeholder={defaultEffort}
        options={[{ value: "", label: defaultEffort }, ...Array.from(new Set([...levels, ...(effort ? [effort] : [])]))
          .map(level => ({ value: level, label: level }))]}
      />
    </LaunchField>}
  </>;
}

export function LaunchLoadState({
  state,
  retry,
  error,
}: {
  state: "loading" | "ready" | "error";
  retry: () => void;
  error?: string | null;
}) {
  const t = useT();
  if (state === "ready") return null;
  return (
    <div
      className="launch-notice"
      role={state === "error" ? "alert" : "status"}
    >
      <span>
        {t(state === "error" ? "launch.optionsError" : "common.loading")}
        {state === "error" && error && <small className="launch-error-detail">{error}</small>}
      </span>
      {state === "error" && (
        <button type="button" className="vlx-btn" onClick={retry}>
          {t("common.retry")}
        </button>
      )}
    </div>
  );
}

export function WorktreeChoices({
  value,
  onChange,
  single = false,
  workflow = false,
  disabled = false,
}: {
  value: WorktreeMode;
  onChange: (v: WorktreeMode) => void;
  single?: boolean;
  workflow?: boolean;
  disabled?: boolean;
}) {
  const t = useT();
  const name = useId();
  return (
    <fieldset className="launch-worktrees" disabled={disabled}>
      <legend className="vlx-field-label">{t("launch.directory")}</legend>
      <div className={`launch-worktree-grid${single ? " is-single" : ""}`}>
        {(["none", ...(single ? [] : ["shared"]), "each"] as const).map(
          (mode) => (
            <label
              key={mode}
              className={`launch-choice${value === mode ? " is-selected" : ""}`}
            >
              <input
                type="radio"
                name={name}
                checked={value === mode}
                onChange={() => onChange(mode as "none" | "shared" | "each")}
              />
              <span>
                <strong>
                  {t(
                    mode === "none"
                      ? "orch.worktreeNone"
                      : mode === "shared"
                        ? "orch.worktreeShared"
                        : single
                          ? "spawn.worktreeLabel"
                          : "orch.worktreeEach",
                  )}
                </strong>
                <span className="launch-hint">
                  {t(
                    mode === "none"
                      ? "launch.directoryCurrentHint"
                      : mode === "shared"
                        ? workflow ? "launch.workflowDirectorySharedHint" : "launch.directorySharedHint"
                        : workflow ? "launch.workflowDirectoryEachHint" : "launch.directoryEachHint",
                  )}
                </span>
              </span>
            </label>
          ),
        )}
      </div>
      <p className="launch-hint">{t(workflow ? "launch.planExecuteWorktreeHint" : "launch.worktreeHint")}</p>
    </fieldset>
  );
}
