import { useState } from "react";
import Select from "./Select";
import { useT, type I18nKey } from "../i18n";
import { useAgentPermissions, savePermissionDefault } from "../hooks/useAgentPermissions";
import type { SessionKind } from "../types";

export function permissionLabelKey(mode: string): I18nKey {
  if (mode === "read-only") return "chat.mode.readOnly";
  if (mode === "full-access") return "chat.mode.fullAccess";
  return `chat.mode.${mode}` as I18nKey;
}

/** OMP and Antigravity defer to their own approval settings in `default` mode. */
export function permissionLabelKeyFor(kind: SessionKind, mode: string): I18nKey {
  return (kind === "omp" || kind === "antigravity") && mode === "default"
    ? "chat.mode.agentDefault"
    : permissionLabelKey(mode);
}

export function AgentPermissionSelect({ kind, value, onChange, width, disabled = false, defaultMode, defaultLabel, keepLabel }: {
  kind: SessionKind;
  value?: string | null;
  onChange: (mode: string) => void | Promise<void>;
  width?: number | string;
  disabled?: boolean;
  defaultMode?: string | null;
  defaultLabel?: string;
  keepLabel?: string;
}) {
  const t = useT();
  const state = useAgentPermissions(kind, value);
  const defaultState = useAgentPermissions(kind, defaultMode ?? null);
  const resolvedDefault = defaultMode ? defaultState?.catalog?.selected : undefined;
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  return <div onKeyDown={event => {
    // Selecting a permission with Enter must not also submit the surrounding session form.
    if (event.defaultPrevented) event.stopPropagation();
  }}>
    <Select value={state?.catalog?.selected ?? ""}
      width={width}
      ariaLabel={t("info.permission")}
      // Dialogs place this field near their bottom edge; portaling lets the list flip above the
      // trigger instead of running past the window.
      menuPortal
      placeholder={state?.error ? "—" : t("common.loading")}
      disabled={disabled || busy || !state?.catalog}
      options={state?.catalog?.modes.map(mode => ({
        value: mode,
        label: t(permissionLabelKeyFor(kind, mode)),
        tag: resolvedDefault && mode === resolvedDefault ? defaultLabel : undefined,
      })) ?? []}
      keepLabel={keepLabel}
      onKeep={keepLabel ? (mode) => {
        if (!mode) return;
        void savePermissionDefault(kind, mode).catch((e) => setError(String(e)));
      } : undefined}
      onChange={mode => {
        setBusy(true); setError(undefined);
        void Promise.resolve().then(() => onChange(mode)).catch(e => setError(String(e))).finally(() => setBusy(false));
      }} />
    {(error || state?.error) && <div role="alert">{error || state?.error}</div>}
  </div>;
}
