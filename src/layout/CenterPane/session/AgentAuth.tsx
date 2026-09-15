import { useEffect, useRef, useState } from "react";
import { useT } from "../../../i18n";
import { chatAuthCancel, chatAuthLogout, chatAuthStart, chatAuthSubmit, type ChatAuthState } from "../../../ipc/chat";
import { copyText } from "../../../ipc/transport";
import { platform } from "../../../platform";
import Icons from "../../../components/Icons";
import { ChipPopover } from "./extras";

type AuthProps = { provider: "Claude" | "Codex"; sessionId: string; state?: ChatAuthState; busy: boolean };

/** True while a sign-in or sign-out is unresolved; the account chip yields to the inline panel then. */
export function needsAttention(state?: ChatAuthState) {
  return state !== undefined && state.status !== "success" && state.status !== "canceled";
}

/** Only unresolved authentication belongs in the conversation. */
export function AgentAuth(props: AuthProps) {
  return needsAttention(props.state) ? <AuthPanel {...props} /> : null;
}

/**
 * Manual account actions stay folded into the composer menu. While a sign-in or sign-out is unresolved
 * the chip stays in the row but disabled: the inline panel above the composer owns that flow.
 */
export function AgentAccountMenu(props: AuthProps) {
  const t = useT();
  return <ChipPopover key={props.state?.status ?? "idle"} glyph={<Icons.lock size={14} />}
    label={t("chat.auth.title", props.provider)} title={t("chat.auth.title", props.provider)} width={320} fitViewport
    disabled={needsAttention(props.state)}>
    <div className="sv-auth-menu"><AuthPanel {...props} state={undefined} /></div>
  </ChipPopover>;
}

/** Login progress comes from the backend, including after a reload or a pane switch. */
function AuthPanel({ provider, sessionId, state, busy }: AuthProps) {
  const t = useT();
  const inFlight = useRef(false);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmLogout, setConfirmLogout] = useState(false);
  const [code, setCode] = useState("");
  const active = state?.status === "starting" || state?.status === "pending" || state?.status === "submitting" || state?.status === "canceling";
  const signingOut = state?.status === "signingOut";
  const signedOut = state?.status === "signedOut";

  useEffect(() => { setConfirmLogout(false); if (state?.status !== "pending") setCode(""); }, [state?.status, busy]);

  async function run(action: () => Promise<void>) {
    if (inFlight.current) return;
    inFlight.current = true;
    setSending(true);
    setError(null);
    try { await action(); }
    catch (err) {
      const message = String(err);
      setError(message === "codex_auth_logout_failed" || message === "claude_auth_logout_failed" ? t("chat.auth.logoutFailed")
        : message === "claude_auth_invalid_code" ? t("chat.auth.claude.invalidCode") : message);
    }
    finally { inFlight.current = false; setSending(false); }
  }

  return <section className={`sv-auth${state ? " sv-auth-panel" : ""}`} aria-label={t("chat.auth.title", provider)}>
    {state && <p role="status">{state.status === "required" || state.status === "signedOut" ? t(`chat.auth.${state.status}`, provider)
      : provider === "Claude" && (state.status === "pending" || state.status === "failed") ? t(`chat.auth.claude.${state.status}`)
        : t(`chat.auth.${state.status}`)}</p>}
    {(state?.status === "required" || active || !state) && <p className="sv-auth-hint">{t("chat.auth.scope", provider)}</p>}
    {state?.status === "pending" && state.verificationUrl && <div className="sv-auth-actions">
      {state.userCode && <><code className="sv-auth-code">{state.userCode}</code>
      <button type="button" disabled={sending} onClick={() => void run(() => copyText(state.userCode!, { reportFailure: true }))}>{t("common.copy")}</button></>}
      <a href={state.verificationUrl} target="_blank" rel="noopener noreferrer" onClick={(event) => {
        if (!event.ctrlKey && !event.metaKey && !event.shiftKey && event.button === 0) {
          event.preventDefault();
          void run(() => platform.opener.openExternal(state.verificationUrl!));
        }
      }}>{t("chat.auth.open")}</a>
    </div>}
    {provider === "Claude" && <p className="sv-auth-hint">{t("chat.auth.claude.externalAuth")}</p>}
    {provider === "Claude" && state?.status === "pending" && <form className="sv-auth-form" onSubmit={(event) => {
      event.preventDefault();
      if (!code.trim() || sending) return;
      const submitted = code.trim();
      setCode("");
      void run(() => chatAuthSubmit(sessionId, submitted));
    }}>
      <label htmlFor={`auth-code-${sessionId}`}>{t("chat.auth.claude.code")}</label>
      <div className="sv-auth-actions">
        <input id={`auth-code-${sessionId}`} type="password" autoComplete="off" spellCheck={false}
          value={code} disabled={sending} onChange={(event) => setCode(event.target.value)} />
        <button type="submit" disabled={sending || !code.trim()}>{t("chat.auth.claude.submit")}</button>
      </div>
    </form>}
    <div className="sv-auth-actions">
      {active
        ? <button type="button" disabled={sending || state?.status === "submitting" || state?.status === "canceling"} onClick={() => void run(() => chatAuthCancel(sessionId))}>{t("common.cancel")}</button>
        : <>
          <button type="button" disabled={sending || busy || signingOut} title={busy ? t("chat.auth.wait") : undefined} onClick={() => {
            setConfirmLogout(false);
            void run(() => chatAuthStart(sessionId));
          }}>{t(signedOut ? "chat.auth.login" : "chat.auth.start")}</button>
          {!signedOut && <button type="button" disabled={sending || busy || signingOut} title={busy ? t("chat.auth.wait") : undefined} onClick={() => {
            setError(null);
            setConfirmLogout(true);
          }}>{t("chat.auth.logout")}</button>}
        </>}
    </div>
    {confirmLogout && !active && !signingOut && !signedOut && <div className="sv-auth-confirm">
      <p>{t("chat.auth.logoutConfirm", provider)}</p>
      <div className="sv-auth-actions">
        <button type="button" disabled={sending || busy} onClick={() => {
          setConfirmLogout(false);
          void run(() => chatAuthLogout(sessionId));
        }}>{t("chat.auth.confirmLogout")}</button>
        <button type="button" disabled={sending} onClick={() => setConfirmLogout(false)}>{t("common.cancel")}</button>
      </div>
    </div>}
    {error && state?.status !== "logoutFailed" && <p role="alert">{error}</p>}
  </section>;
}
