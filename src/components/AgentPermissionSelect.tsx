import { useState } from "react";
import Select from "./Select";
import { useT, type I18nKey } from "../i18n";
import { useAgentPermissions } from "../hooks/useAgentPermissions";
import type { SessionKind } from "../types";

export function permissionLabelKey(mode: string): I18nKey {
  if (mode === "read-only") return "chat.mode.readOnly";
  if (mode === "full-access") return "chat.mode.fullAccess";
  if (mode === "build") return "chat.mode.default";
  if (mode === "edit") return "chat.mode.acceptEdits";
  if (mode === "yolo") return "chat.mode.bypassPermissions";
  return `chat.mode.${mode}` as I18nKey;
}

export function AgentPermissionSelect({ kind, value, onChange }: {
  kind: SessionKind; value?: string | null; onChange: (mode: string) => void | Promise<void>;
}) {
  const t = useT();
  const state = useAgentPermissions(kind, value);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  return <div onKeyDown={event => {
    // Selecting a permission with Enter must not also submit the surrounding session form.
    if (event.defaultPrevented) event.stopPropagation();
  }}>
    <Select value={state?.catalog?.selected ?? ""}
      ariaLabel={t("info.permission")}
      placeholder={state?.error ? "—" : t("common.loading")}
      disabled={busy || !state?.catalog}
      options={state?.catalog?.modes.map(mode => ({ value: mode, label: t(permissionLabelKey(mode)) })) ?? []}
      onChange={mode => {
        setBusy(true); setError(undefined);
        void Promise.resolve().then(() => onChange(mode)).catch(e => setError(String(e))).finally(() => setBusy(false));
      }} />
    {(error || state?.error) && <div role="alert">{error || state?.error}</div>}
  </div>;
}
