//! DeepSeek's API endpoint and key, set from the conversation's own composer.
//!
//! A DeepSeek conversation runs through OpenCode, which reads its provider configuration when its
//! server starts. The key prefers the system keychain and is never read back, so the field only ever
//! shows whether one is saved. Saving ends idle DeepSeek servers, so the next message uses the new
//! credentials without restarting the app.

import { useState } from "react";

import Icons from "../../../components/Icons";
import { PasswordField } from "../../../components/PasswordField";
import { useT } from "../../../i18n";
import { mapBackendError } from "../../../ipc/backendError";
import { deepseekGetStatus, deepseekSetConfig, type DeepSeekApiStatus } from "../../../ipc/commands";
import { ChipPopover } from "./extras";

export function DeepSeekApiChip() {
  const t = useT();
  // The key lives on the host that runs the agent. The backend reports whether this client may write
  // it, so a remote browser shows the form read-only instead of failing at save time.
  const [status, setStatus] = useState<DeepSeekApiStatus | null>(null);
  const editable = status?.editable ?? true;
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const [msgOk, setMsgOk] = useState(false);

  const load = () => {
    void deepseekGetStatus()
      .then((next) => {
        setStatus(next ?? null);
        setBaseUrl(next?.baseUrl ?? "");
        setApiKey("");
        setMsg(null);
      })
      // A failed status read leaves the form usable; the save itself reports any real problem.
      .catch(() => {});
  };

  const save = async (clearKey = false) => {
    if (!editable || busy) return;
    setBusy(true);
    setMsg(null);
    try {
      const next = await deepseekSetConfig(baseUrl, clearKey ? "" : apiKey, clearKey);
      setStatus(next ?? null);
      setBaseUrl(next?.baseUrl ?? "");
      setApiKey("");
      setMsgOk(true);
      setMsg(t("settings.deepseekSaved"));
    } catch (error) {
      // A stringified rejection carries an `Error: ` prefix that hides the stable code below it.
      const raw = (error instanceof Error ? error.message : String(error)).replace(/^Error: /, "");
      setMsgOk(false);
      setMsg(mapBackendError(raw));
    } finally {
      setBusy(false);
    }
  };

  return (
    <ChipPopover
      glyph={<Icons.key size={14} />}
      label="API"
      title={t("settings.deepseekApiTitle")}
      width={340}
      maxHeight={420}
      onOpen={load}
    >
      <form
        className="sv-deepseek-api"
        onSubmit={(event) => {
          event.preventDefault();
          void save(false);
        }}
      >
        <div className="sv-deepseek-api-title">{t("settings.deepseekApiTitle")}</div>
        <p className="sv-deepseek-api-desc">{t("settings.deepseekApiDesc")}</p>
        {!editable && <p className="sv-deepseek-api-desc">{t("settings.deepseekLocalOnly")}</p>}
        <div className="sv-deepseek-api-field">
          <span>{t("settings.deepseekBaseUrl")}</span>
          <input
            className="vlx-input"
            type="url"
            value={baseUrl}
            onChange={(event) => setBaseUrl(event.target.value)}
            placeholder="https://api.deepseek.com"
            spellCheck={false}
            disabled={busy || !editable}
            aria-label={t("settings.deepseekBaseUrl")}
          />
        </div>
        <div className="sv-deepseek-api-field">
          <span>{t("settings.deepseekApiKey")}</span>
          <PasswordField
            value={apiKey}
            onChange={setApiKey}
            placeholder={
              status?.hasKey ? t("settings.deepseekKeySaved") : t("settings.deepseekKeyPlaceholder")
            }
            ariaLabel={t("settings.deepseekApiKey")}
            disabled={busy || !editable}
            inputStyle={{ padding: "5px 8px", fontSize: "calc(var(--ui-fs) - 1px)" }}
          />
        </div>
        <p className="sv-deepseek-api-hint">{t("settings.deepseekEndpointHint")}</p>
        <div className="sv-deepseek-api-actions">
          {status?.hasKey && (
            <button
              className="vlx-btn"
              type="button"
              disabled={busy || !editable}
              onClick={() => void save(true)}
            >
              {t("settings.deepseekRemoveKey")}
            </button>
          )}
          <button className="vlx-btn vlx-btn-primary" type="submit" disabled={busy || !editable}>
            {t("common.save")}
          </button>
        </div>
        {msg && (
          <p
            className={"sv-deepseek-api-msg" + (msgOk ? "" : " sv-deepseek-api-error")}
            role={msgOk ? "status" : "alert"}
          >
            {msg}
          </p>
        )}
      </form>
    </ChipPopover>
  );
}
