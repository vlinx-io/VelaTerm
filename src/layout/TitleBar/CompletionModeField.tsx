import { useState } from "react";
import { useT } from "../../i18n";
import { setCompletionMode, setCompletionTabBehavior, useCompletionConfig, type TabBehavior } from "../../terminal/completion/config";
import { Field, Seg } from "./settingsParts";

export function CompletionModeField() {
  const t = useT();
  const config = useCompletionConfig();
  const [saving, setSaving] = useState(false);
  const [failed, setFailed] = useState(false);
  // Hide the setting on a host without the shell integration (Windows for now) instead of leaving a switch that
  // changes nothing, whatever value it holds.
  if (config && !config.available) return null;
  const save = (write: () => Promise<void>) => {
    setSaving(true); setFailed(false);
    void write().catch(() => setFailed(true)).finally(() => setSaving(false));
  };
  const tabBehaviors: TabBehavior[] = ["complete", "accept"];
  return <>
    <Field label={t("settings.completionMode")}>
      {config ? <Seg value={config.mode}
        options={config.modes.map(mode => [mode, t(mode === "auto" ? "settings.completionAuto" : mode === "tab" ? "settings.completionTab" : "settings.completionOff")])}
        disabledOptions={saving ? config.modes : []}
        onChange={mode => save(() => setCompletionMode(mode))} /> : <span role="status">{t("settings.completionUnavailable")}</span>}
    </Field>
    <p style={{ fontSize: 11, color: "var(--text-dim)", margin: "6px 0 12px" }}>
      {failed ? t("settings.completionUnavailable")
        : config?.mode === "tab" ? t("settings.completionTabOnlyHint")
        : config?.mode === "off" ? null : t("settings.completionHint")}
    </p>
    {/* The Tab choice only concerns the inline hint, which exists in this mode alone. */}
    {config?.mode === "auto" && <>
      <Field label={t("settings.completionTabKey")}>
        <Seg value={config.tabBehavior}
          options={[["complete", t("settings.completionTabComplete")], ["accept", t("settings.completionTabAccept")]]}
          disabledOptions={saving ? tabBehaviors : []}
          onChange={behavior => save(() => setCompletionTabBehavior(behavior))} />
      </Field>
      <p style={{ fontSize: 11, color: "var(--text-dim)", margin: "6px 0 12px" }}>
        {t(config.tabBehavior === "accept" ? "settings.completionTabAcceptHint" : "settings.completionTabCompleteHint")}
      </p>
    </>}
  </>;
}
