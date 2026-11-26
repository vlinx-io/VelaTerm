//! Reusable form fields for reviewing child-session launch requests.

import { useEffect, useId, useMemo, useState, type ReactNode } from "react";
import { useT } from "../i18n";
import { onAntigravityModelsChanged } from "../ipc/events";
import { launchModels, launchOptions, type LaunchOption, type LaunchModelContext, type LaunchModelCatalog } from "../ipc/launch";
import type { WorktreeMode } from "../ipc/events";
import { LevelBar } from "../layout/CenterPane/session/controls";
import {
  EFFORTS,
  THINKING_NONE,
  THINKING_OFF,
  chipToSelectOption,
  effortChipLabel,
  effortChipOptions,
  levelsOf,
  modelChipOptions,
} from "../layout/CenterPane/session/modelChips";
import { assistantLabel, kindIconEl } from "../layout/sessionViewers/sessionMeta";
import type { SessionKind } from "../types";
import { Field } from "./Field";
import { Icons } from "./Icons";
import Select from "./Select";
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
  as,
}: {
  label: string;
  children: ReactNode;
  hint?: string;
  as?: "label" | "div";
}) {
  return <Field label={label} hint={hint} as={as}>{children}</Field>;
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
  const key = kind
    ? JSON.stringify([kind, context.parentSessionId || null, context.cwd || null, context.execPath || null, !!context.inheritArgs])
    : "";
  const [response, setResponse] = useState<{ key: string; catalog?: LaunchModelCatalog; error?: string } | null>(null);
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    if (kind !== "antigravity") return;
    let active = true;
    let stop: (() => void) | null = null;
    void onAntigravityModelsChanged(() => {
      if (active) setRevision((r) => r + 1);
    }).then((unlisten) => {
      if (active) stop = unlisten;
      else unlisten();
    }).catch(() => {});
    return () => {
      active = false;
      stop?.();
    };
  }, [kind]);
  useEffect(() => {
    let live = true;
    setResponse(null);
    if (!key) return;
    // Directory editing should not start a discovery process for every keystroke.
    const timer = setTimeout(() => {
      const [kind, parentSessionId, cwd, execPath, inheritArgs] = JSON.parse(key) as [string, string | null, string | null, string | null, boolean];
      void models(kind, { parentSessionId, cwd, ...(execPath ? { execPath } : {}), inheritArgs }).then(catalog => {
        if (live) setResponse({ key, catalog });
      }).catch(cause => { if (live) setResponse({ key, error: launchErrorText(cause) }); });
    }, 180);
    return () => { live = false; clearTimeout(timer); };
  }, [key, revision]);
  const current = key && response?.key === key ? response : null;
  return { current, catalog: current?.catalog, retry: () => setRevision(r => r + 1) };
}

/** Resolve the raw native effort levels offered for a specific model or fallback spec. */
export function rawEffortLevelsForModel(
  kind: SessionKind,
  spec: LaunchOption,
  catalog: LaunchModelCatalog | undefined,
  modelsFailed: boolean,
  targetModel: string,
  conversationEfforts = false,
): readonly string[] {
  const entry = targetModel ? catalog?.models.find((m) => m.id === targetModel) : undefined;
  if (entry) return entry.effortLevels;
  if (catalog && !modelsFailed) {
    if (catalog.effortLevels.length > 0) return catalog.effortLevels;
    if (catalog.models.length > 0) return [];
  }
  return spec.effortLevels.length
    ? spec.effortLevels
    : spec.effortFlag || (conversationEfforts && kind === "zcode")
      ? EFFORTS
      : [];
}

/** Build the ordered effort chip options for a model, optionally including chat-only Claude levels. */
export function buildEffortChips(
  t: ReturnType<typeof useT>,
  kind: SessionKind,
  spec: LaunchOption,
  catalog: LaunchModelCatalog | undefined,
  modelsFailed: boolean,
  targetModel: string,
  conversationEfforts = false,
  defaultEffortLabel?: string,
) {
  const raw = rawEffortLevelsForModel(kind, spec, catalog, modelsFailed, targetModel, conversationEfforts);
  const allowed = conversationEfforts ? levelsOf(raw, kind) : [...raw];
  const includeOff = (conversationEfforts && kind === "claude") || allowed.includes(THINKING_OFF) || allowed.includes(THINKING_NONE);
  const chips = effortChipOptions(t, allowed, true, includeOff);
  return defaultEffortLabel
    ? chips.map((c) => (c.value === "" ? { ...c, label: defaultEffortLabel } : c))
    : chips;
}

/** A model and its effort share one native catalogue and one atomic draft update. */
export function ModelEffortFields({
  spec, model, effort, onChange, onModelCommit, onEffortCommit,
  context = {}, modelPlaceholder, effortPlaceholder, inheritedModel, disabled = false,
  models: modelsProp, conversationEfforts = false, defaultModel, defaultEffortForModel,
  resolveEffortForModel, onKeep,
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
  /** Reuse a parent-fetched model catalogue so New Session avoids duplicate lookups. */
  models?: ReturnType<typeof useLaunchModels>;
  /** Include interactive conversation-only levels (Claude Off / Ultra Code and ZCode effort before load). */
  conversationEfforts?: boolean;
  /** Model ID tagged as the saved app default in the model menu. */
  defaultModel?: string;
  /** Effort level tagged as the saved app default for the inspected model. */
  defaultEffortForModel?: (targetModel: string) => string;
  /** Optional custom resolver calculating the next effort level when switching models. */
  resolveEffortForModel?: (targetModel: string) => string;
  /** Enable "Set as default" persistence in both the model and reasoning-effort popups. */
  onKeep?: (model: string, effort: string) => void;
}) {
  const t = useT();
  const kind = spec.id as SessionKind;
  const internalModels = useLaunchModels(modelsProp ? null : spec.id, context);
  const { current, catalog, retry } = modelsProp ?? internalModels;
  const catalogModels = useMemo(() => catalog?.models ?? [], [catalog]);
  const modelsLoading = !current;
  const modelsFailed = Boolean(current?.error);
  const effectiveModel = model || inheritedModel || "";
  const selectedModel = effectiveModel ? catalogModels.find((entry) => entry.id === effectiveModel) : undefined;
  const savedDefaultTag = t("chat.savedModelDefault");
  const defaultModelLabel = modelPlaceholder || t("chat.followModelDefault", assistantLabel(kind));

  const modelSelectOptions = useMemo(() => {
    const visibleModels = modelsFailed ? [] : catalogModels;
    // Disambiguate colliding model display names across providers with the raw identifier.
    const labelCounts = new Map<string, number>();
    for (const entry of visibleModels) {
      labelCounts.set(entry.label, (labelCounts.get(entry.label) ?? 0) + 1);
    }
    const base = modelChipOptions(t, kind, visibleModels, defaultModelLabel).map((opt) => {
      const option = chipToSelectOption(opt, defaultModel, savedDefaultTag);
      if (opt.value && (labelCounts.get(opt.label) ?? 0) > 1 && opt.value !== opt.label) {
        return { ...option, hint: opt.value };
      }
      return option;
    });
    if (model && !base.some((o) => o.value === model)) {
      base.push({
        value: model,
        label: model,
        icon: kindIconEl(kind, 14),
        tag: defaultModel && model === defaultModel ? savedDefaultTag : undefined,
      });
    }
    return base;
  }, [t, kind, modelsFailed, catalogModels, defaultModelLabel, defaultModel, savedDefaultTag, model]);

  const effortSelectOptions = useMemo(() => {
    const chips = buildEffortChips(
      t, kind, spec, catalog, modelsFailed, effectiveModel, conversationEfforts, effortPlaceholder,
    );
    const valid = new Set(chips.map((o) => o.value));
    const remembered = defaultEffortForModel?.(effectiveModel) ?? "";
    const defaultTagValue = remembered && valid.has(remembered) ? remembered : "";
    const base = chips.map((opt) => chipToSelectOption(opt, defaultTagValue, savedDefaultTag));
    // Preserve uncatalogued caller or restored drafts while loading, after error, or for uncatalogued models.
    if (effort && (!catalog || modelsFailed || !selectedModel) && !base.some((o) => o.value === effort)) {
      const idx = (EFFORTS as readonly string[]).indexOf(effort);
      base.push({
        value: effort,
        label: effortChipLabel(t, effort),
        icon: idx >= 0 ? <LevelBar level={idx + 1} of={EFFORTS.length} /> : undefined,
      });
    }
    return base;
  }, [
    t, kind, spec, catalog, modelsFailed, effectiveModel, conversationEfforts,
    effortPlaceholder, defaultEffortForModel, savedDefaultTag, effort, selectedModel,
  ]);

  const nextEffortFor = (nextModel: string) => {
    if (resolveEffortForModel) return resolveEffortForModel(nextModel);
    if (effort && catalog && !modelsFailed && catalogModels.some((entry) => entry.id === nextModel)) {
      const nextValid = new Set(
        buildEffortChips(
          t, kind, spec, catalog, modelsFailed, nextModel, conversationEfforts, effortPlaceholder,
        ).map((o) => o.value),
      );
      return nextValid.has(effort) ? effort : "";
    }
    return effort;
  };

  const pickModel = (nextModel: string) => {
    if (disabled) return;
    const nextEffort = nextEffortFor(nextModel);
    onChange(nextModel, nextEffort);
    if (nextEffort !== effort) onEffortCommit?.(nextEffort);
    onModelCommit?.(nextModel);
  };

  const pickEffort = (nextEffort: string) => {
    if (disabled) return;
    onChange(model, nextEffort);
    onEffortCommit?.(nextEffort);
  };

  const showEffort = Boolean(
    spec.effortFlag || spec.effortLevels.length || catalog?.effortLevels.length || (conversationEfforts && kind === "zcode"),
  );

  return <>
    <div className="launch-field">
      <LaunchField label={t("spawn.modelLabel")}>
        <Select
          key={`${spec.id}:${String(disabled)}`}
          value={model}
          width="100%"
          menuPortal
          disabled={disabled}
          ariaLabel={t("spawn.modelLabel")}
          searchable
          searchPlaceholder={t("chat.filterPlaceholder")}
          noMatchLabel={t("tree.noMatch")}
          options={modelSelectOptions}
          customEntry={{
            label: (q) => t("settings.fontUseCustom", q),
            when: (q) => q.length > 0 && !/\s/.test(q) && !modelSelectOptions.some((o) => o.value === q),
            onSelect: pickModel,
          }}
          keepLabel={onKeep && !modelsFailed && catalogModels.length > 0 ? t("chat.keepChoice") : undefined}
          onKeep={onKeep ? (picked) => onKeep(picked, picked === model ? effort : nextEffortFor(picked)) : undefined}
          footer={(modelsLoading || modelsFailed || catalogModels.length === 0) ? (
            <div className={"launch-select-status" + (modelsFailed ? " is-error" : "")} role="status">
              <span>{t(modelsLoading ? "common.loading" : modelsFailed ? "chat.modelsLoadFailed" : "chat.modelsEmpty")}</span>
              {!modelsLoading && <button type="button" className="launch-link" onClick={retry}>{t("common.retry")}</button>}
            </div>
          ) : undefined}
          onChange={pickModel}
        />
      </LaunchField>
      {current?.error && <div className="launch-hint" role="alert">
        <div className="launch-inline"><span>{t("chat.modelsLoadFailed")}</span>
          <button type="button" className="launch-link" onClick={retry}>{t("common.retry")}</button>
        </div>
        <span className="launch-error-detail">{current.error}</span>
      </div>}
    </div>
    {showEffort && <LaunchField label={t("spawn.effortLabel")}>
      <Select
        key={`${spec.id}:${String(disabled)}`}
        value={effort}
        width="100%"
        menuPortal
        disabled={disabled || effortSelectOptions.length <= 1}
        ariaLabel={t("spawn.effortLabel")}
        options={effortSelectOptions}
        keepLabel={onKeep ? (selectedModel ? t("chat.keepChoiceFor", selectedModel.label) : t("chat.keepChoice")) : undefined}
        onKeep={onKeep ? (picked) => onKeep(model, picked) : undefined}
        onChange={pickEffort}
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
  allowShared = true,
  disabled = false,
}: {
  value: WorktreeMode;
  onChange: (v: WorktreeMode) => void;
  single?: boolean;
  workflow?: boolean;
  allowShared?: boolean;
  disabled?: boolean;
}) {
  const t = useT();
  const name = useId();
  const twoColumn = single || !allowShared;
  return (
    <fieldset className="launch-worktrees" disabled={disabled}>
      <legend className="vlx-field-label">{t("launch.directory")}</legend>
      <div className={`launch-worktree-grid${twoColumn ? " is-single" : ""}`}>
        {(["none", ...(twoColumn ? [] : ["shared"]), "each"] as const).map(
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
