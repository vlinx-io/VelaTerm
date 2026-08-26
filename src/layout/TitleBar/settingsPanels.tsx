//! Category panels for Settings: self-contained Shortcuts, Agents, and Gitea integration panels,
//! including their private controls and constants. Extracted from the growing SettingsModal; the main
//! settings page renders the selected category. Shared Seg, Field, and SectionTitle live in settingsParts.

import { useEffect, useState } from "react";
import { normalizeArgDashes } from "../../args";
import Select from "../../components/Select";
import { useT, type I18nKey } from "../../i18n";
import {
  DEFAULT_BINDINGS,
  SHORTCUT_ACTIONS,
  comboFromEvent,
  formatCombo,
  type ShortcutAction,
} from "../../hooks/shortcutRegistry";
import {
  giteaGetStatus,
  giteaProbe,
  giteaSetConfig,
  type GiteaStatus,
} from "../../ipc/commands";
import { isTauri } from "../../ipc/transport";
import { env } from "../../platform";
import { useTermStore } from "../../store/termStore";
import type { SessionKind } from "../../types";
import { kindIconEl } from "../sessionViewers/sessionMeta";
import { Field, Seg, SectionTitle } from "./settingsParts";

/** i18n label keys for actions with configurable shortcuts. */
const SC_LABEL: Record<ShortcutAction, I18nKey> = {
  openProject: "settings.scOpenProject",
  newTab: "settings.scNewTab",
  newBrowserTab: "settings.scNewBrowserTab",
  closePane: "settings.scClosePane",
  splitRight: "settings.scSplitRight",
  splitDown: "settings.scSplitDown",
  search: "settings.scSearch",
  globalSearch: "settings.scGlobalSearch",
  saveDoc: "settings.scSaveDoc",
};

/** Shortcut row with a label and current chord button. Clicking the button starts key capture. */
function ShortcutRow({
  label,
  combo,
  recording,
  onStart,
  onCapture,
  onCancel,
}: {
  label: string;
  combo: string;
  recording: boolean;
  onStart: () => void;
  onCapture: (combo: string) => void;
  onCancel: () => void;
}) {
  const t = useT();
  useEffect(() => {
    if (!recording) return;
    // Capture on window before the global document-level shortcut hook and suppress default behavior.
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        onCancel();
        return;
      }
      const c = comboFromEvent(e);
      if (c) onCapture(c); // Only modifier-plus-letter chords are valid; keep waiting on null.
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [recording, onCapture, onCancel]);

  return (
    <Field label={label}>
      <button
        onClick={recording ? onCancel : onStart}
        style={{
          minWidth: 96,
          height: 26,
          padding: "0 12px",
          background: "var(--bg-active)",
          color: recording ? "var(--accent)" : "var(--text)",
          border: `1px solid ${recording ? "var(--accent)" : "var(--border)"}`,
          borderRadius: 6,
          fontSize: 11.5,
          fontVariantNumeric: "tabular-nums",
          cursor: "pointer",
        }}
      >
        {recording ? t("settings.scRecording") : formatCombo(combo)}
      </button>
    </Field>
  );
}

/** Shortcuts category: configurable actions, conflict-aware key capture, and restore defaults. */
export function ShortcutsPanel() {
  const t = useT();
  const overrides = useTermStore((s) => s.shortcutOverrides);
  const setShortcut = useTermStore((s) => s.setShortcut);
  const resetShortcuts = useTermStore((s) => s.resetShortcuts);
  const [recording, setRecording] = useState<ShortcutAction | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const eff = (a: ShortcutAction) => overrides[a] || DEFAULT_BINDINGS[a];

  const capture = (action: ShortcutAction, combo: string) => {
    // Reject a new chord already assigned to another action and leave persisted settings unchanged.
    const clash = SHORTCUT_ACTIONS.find((a) => a !== action && eff(a) === combo);
    setRecording(null);
    if (clash) {
      setErr(t("settings.scConflict", t(SC_LABEL[clash])));
      return;
    }
    setErr(null);
    setShortcut(action, combo);
  };

  // New browser tabs are desktop-only, so hide this row in browser and remote clients.
  const actions = SHORTCUT_ACTIONS.filter((a) => a !== "newBrowserTab" || isTauri || env.isElectron);

  return (
    <>
      <SectionTitle>{t("settings.catShortcuts")}</SectionTitle>
      {actions.map((a) => (
        <ShortcutRow
          key={a}
          label={t(SC_LABEL[a])}
          combo={eff(a)}
          recording={recording === a}
          onStart={() => {
            setErr(null);
            setRecording(a);
          }}
          onCancel={() => setRecording(null)}
          onCapture={(c) => capture(a, c)}
        />
      ))}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: 12,
          marginTop: 14,
        }}
      >
        <span
          style={{
            flex: 1,
            fontSize: 11,
            lineHeight: 1.5,
            color: err ? "#e5484d" : "var(--text-dim)",
          }}
        >
          {err ?? t("settings.scHint")}
        </span>
        <button
          onClick={() => {
            setErr(null);
            setRecording(null);
            resetShortcuts();
          }}
          style={{
            flex: "none",
            padding: "5px 14px",
            fontSize: 11.5,
            borderRadius: 5,
            border: "1px solid var(--border)",
            background: "var(--bg-active)",
            color: "var(--text)",
            cursor: "pointer",
          }}
        >
          {t("settings.scReset")}
        </button>
      </div>
    </>
  );
}

/** Local agent types with configurable defaults, matching `AGENT_ARGS_KINDS` in sessionMenu.
 * Labels are product names and need no localization. `yolo` is the CLI flag appended when skipping
 * all permission prompts; an empty value means the agent has no such flag (OpenCode uses config).
 * Flag descriptions must match backend `inject::permission_flag`. `permVia` identifies the control
 * mechanism: `flag` for most agents, `env` for OpenCode, or `none` for Pi, whose tools run directly.
 * Add future agents here and both the dropdown and configuration area will follow automatically. */
const AGENT_DEFAULT_KINDS: {
  kind: SessionKind;
  label: string;
  yolo: string;
  permVia: "flag" | "env" | "none" | "inverse";
}[] = [
  { kind: "claude", label: "Claude", yolo: "--dangerously-skip-permissions", permVia: "flag" },
  { kind: "codex", label: "Codex", yolo: "--dangerously-bypass-approvals-and-sandbox", permVia: "flag" },
  { kind: "opencode", label: "OpenCode", yolo: "", permVia: "env" },
  { kind: "copilot", label: "Copilot", yolo: "--allow-all-tools", permVia: "flag" },
  { kind: "cursor", label: "Cursor", yolo: "--force", permVia: "flag" },
  // Antigravity (agy) shares Claude's flag name and injects it only in skip mode.
  { kind: "antigravity", label: "Antigravity", yolo: "--dangerously-skip-permissions", permVia: "flag" },
  // Cline is inverted because its native default is fully automatic. Both modes inject an explicit
  // flag: `--auto-approve false` by default and `--auto-approve true` in skip mode (see backend
  // `inject::permission_flag`). `yolo` is display-only here; the backend injects the actual flag.
  { kind: "cline", label: "Cline", yolo: "--auto-approve true", permVia: "flag" },
  { kind: "pi", label: "Pi", yolo: "", permVia: "none" },
  // Crush injects `--yolo` only in skip mode; default mode retains native staged approval.
  { kind: "crush", label: "Crush", yolo: "--yolo", permVia: "flag" },
  { kind: "kimi", label: "Kimi Code (K3)", yolo: "--yolo", permVia: "flag" },
  { kind: "kiro", label: "Kiro", yolo: "--trust-all-tools", permVia: "flag" },
  { kind: "grok", label: "Grok Build (Grok 4.5)", yolo: "--always-approve", permVia: "flag" },
  // Zoo auto-approves natively; VelaTerm injects --require-approval in default mode and no skip flag.
  { kind: "zoo", label: "Zoo Code", yolo: "", permVia: "inverse" },
];

/** Agent picker: each row carries the agent's own icon so the list reads at a glance. */
function AgentSelect({
  value,
  onChange,
}: {
  value: SessionKind;
  onChange: (v: SessionKind) => void;
}) {
  const options = AGENT_DEFAULT_KINDS.map((a) => ({
    value: a.kind,
    label: a.label,
    icon: <span style={{ display: "inline-flex", flex: "none" }}>{kindIconEl(a.kind, 15)}</span>,
  }));
  return <Select value={value} onChange={onChange} options={options} width={200} align="right" />;
}

/** Multiline default-arguments field with a label above a full-width textarea. Edits remain local until
 * blur to avoid writing localStorage on every keystroke. Each line may contain one or more flags; the
 * backend flattens them at launch. Enter adds a line, while Escape reverts without closing the modal. */
function AgentArgsBlock({
  label,
  value,
  onCommit,
}: {
  label: string;
  value: string;
  onCommit: (v: string) => void;
}) {
  const [draft, setDraft] = useState(value);
  // Resynchronize the draft when the stored value changes, such as after switching agents.
  useEffect(() => setDraft(value), [value]);
  return (
    <div style={{ padding: "10px 0", borderBottom: "1px solid var(--border)" }}>
      <div style={{ color: "var(--text-dim)", fontSize: 12.5, marginBottom: 6 }}>{label}</div>
      <textarea
        value={draft}
        rows={4}
        placeholder={"--model opus\n--add-dir ./src"}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={() => onCommit(normalizeArgDashes(draft))}
        onKeyDown={(e) => {
          // Escape reverts; stop propagation so it does not close Settings. Enter remains a newline.
          if (e.key === "Escape") {
            e.stopPropagation();
            setDraft(value);
          }
        }}
        style={{
          width: "100%",
          minHeight: 76,
          padding: "8px 10px",
          background: "var(--bg-active)",
          color: "var(--text)",
          border: "1px solid var(--border)",
          borderRadius: 6,
          fontSize: 12,
          fontFamily: "var(--font-mono, monospace)",
          lineHeight: 1.5,
          outline: "none",
          resize: "vertical",
          boxSizing: "border-box",
        }}
      />
    </div>
  );
}

/** Single-line absolute executable path, global per agent type (see `AgentDefaultConfig.path`). Edits
 * remain local until blur or Enter; Escape reverts without closing the modal. Empty preserves PATH
 * lookup by command name, while a value makes the backend spawn that exact path. This supports
 * installations outside PATH, and successful one-click installation fills it automatically. */
function AgentPathBlock({
  label,
  hint,
  placeholder,
  value,
  onCommit,
}: {
  label: string;
  hint: string;
  placeholder: string;
  value: string;
  onCommit: (v: string) => void;
}) {
  const [draft, setDraft] = useState(value);
  // Resynchronize the draft after agent changes or automatic updates from the installer card.
  useEffect(() => setDraft(value), [value]);
  return (
    <div style={{ padding: "10px 0", borderBottom: "1px solid var(--border)" }}>
      <div style={{ color: "var(--text-dim)", fontSize: 12.5, marginBottom: 6 }}>{label}</div>
      <input
        type="text"
        value={draft}
        placeholder={placeholder}
        spellCheck={false}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={() => onCommit(draft.trim())}
        onKeyDown={(e) => {
          // Escape reverts without closing Settings; Enter commits through the blur handler.
          if (e.key === "Escape") {
            e.stopPropagation();
            setDraft(value);
          } else if (e.key === "Enter") {
            e.currentTarget.blur();
          }
        }}
        style={{
          width: "100%",
          padding: "7px 10px",
          background: "var(--bg-active)",
          color: "var(--text)",
          border: "1px solid var(--border)",
          borderRadius: 6,
          fontSize: 12,
          fontFamily: "var(--font-mono, monospace)",
          outline: "none",
          boxSizing: "border-box",
        }}
      />
      <div style={{ marginTop: 6, fontSize: 11, lineHeight: 1.5, color: "var(--text-dim)" }}>
        {hint}
      </div>
    </div>
  );
}

/** Agents category. Select an agent, then configure its executable path, default launch arguments,
 * and default permission mode in `store.agentDefaults`. New sessions use these when no explicit value
 * is supplied. Per-session values override defaults, except executable paths, which are always global. */
export function AgentsPanel() {
  const t = useT();
  const agentDefaults = useTermStore((s) => s.agentDefaults);
  const setAgentDefault = useTermStore((s) => s.setAgentDefault);
  const [selKind, setSelKind] = useState<SessionKind>("claude");
  const meta = AGENT_DEFAULT_KINDS.find((a) => a.kind === selKind);
  const supportsYolo =
    !!meta?.yolo || meta?.permVia === "env" || meta?.permVia === "inverse";
  const cfg = agentDefaults[selKind] ?? {};
  const skip = cfg.permissionMode === "skip";
  const isEnvPerm = meta?.permVia === "env";

  return (
    <>
      <SectionTitle>{t("settings.catAgents")}</SectionTitle>

      <Field label={t("resume.agentType")}>
        <AgentSelect value={selKind} onChange={setSelKind} />
      </Field>

      {/* The key includes selKind so switching agents rebuilds the input, resetting the draft to the new agent's current value. */}
      <AgentPathBlock
        key={`path-${selKind}`}
        label={t("settings.agentPathLabel")}
        hint={t("settings.agentPathHint")}
        placeholder={t("settings.agentPathPlaceholder")}
        value={cfg.path ?? ""}
        onCommit={(v) => setAgentDefault(selKind, { path: v })}
      />

      <AgentArgsBlock
        key={selKind}
        label={t("tree.agentArgsLabel")}
        value={cfg.args ?? ""}
        onCommit={(v) => setAgentDefault(selKind, { args: v })}
      />

      <Field label={t("info.permission")}>
        {supportsYolo ? (
          <Seg<"default" | "skip">
            value={skip ? "skip" : "default"}
            options={[
              ["default", t("settings.permDefault")],
              ["skip", t("settings.permYolo")],
            ]}
            onChange={(v) => setAgentDefault(selKind, { permissionMode: v })}
          />
        ) : (
          // With neither a flag nor environment injection, show disabled guidance instead of a toggle.
          <span
            style={{
              fontSize: 11,
              lineHeight: 1.45,
              color: "var(--text-dim)",
              maxWidth: 260,
              textAlign: "right",
            }}
          >
            {meta?.permVia === "none"
              ? t("tree.permissionUnsupportedPi")
              : t("tree.permissionUnsupported")}
          </span>
        )}
      </Field>

      {/* Choosing YOLO shows a notice: flag-based agents display the exact CLI flag, env-based ones explain that it is injected through configuration. */}
      {supportsYolo && skip && meta && meta.permVia !== "inverse" && (
        <div
          style={{ marginTop: 10, fontSize: 11, lineHeight: 1.5, color: "var(--text-dim)" }}
        >
          {isEnvPerm
            ? t("settings.permViaEnvHint")
            : t("settings.yoloHint", meta.yolo)}
        </div>
      )}

      <div
        style={{ marginTop: 12, fontSize: 11, lineHeight: 1.5, color: "var(--text-dim)" }}
      >
        {t("settings.agentArgsHint")}
      </div>
    </>
  );
}

/** Visibility switch for Gitea integration. Set true to restore the currently hidden settings card. */
export const SHOW_GITEA_INTEGRATION = false;

/** Gitea integration card in General for the worktree-backed Gitea PR provider. Configure a base URL
 * and token, test the connection, and save. Tokens use the system keychain with plaintext fallback.
 * Stored tokens are represented by an empty placeholder and never echoed; leaving it blank preserves
 * the existing token while allowing the base URL to change. */
export function GiteaIntegrationPanel() {
  const t = useT();
  const [status, setStatus] = useState<GiteaStatus | null>(null);
  const [baseUrl, setBaseUrl] = useState("");
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState("");
  const [msgOk, setMsgOk] = useState(false);

  useEffect(() => {
    let alive = true;
    void giteaGetStatus()
      .then((s) => {
        if (!alive) return;
        setStatus(s);
        setBaseUrl(s.baseUrl ?? "");
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  const test = async () => {
    setBusy(true);
    setMsg("");
    try {
      const p = await giteaProbe(baseUrl.trim(), token.trim());
      setMsgOk(p.ok);
      setMsg(p.message);
    } catch (e) {
      setMsgOk(false);
      setMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  const save = async () => {
    setBusy(true);
    setMsg("");
    try {
      const s = await giteaSetConfig(baseUrl.trim(), token.trim());
      setStatus(s);
      setToken(""); // Clear after saving so the stored placeholder appears without exposing the token.
      setMsgOk(true);
      setMsg(t("gitea.saved"));
    } catch (e) {
      setMsgOk(false);
      setMsg(String(e));
    } finally {
      setBusy(false);
    }
  };

  const inputStyle: React.CSSProperties = {
    width: 240,
    boxSizing: "border-box",
  };

  return (
    <>
      <SectionTitle>{t("gitea.title")}</SectionTitle>
      <div style={{ fontSize: 11, lineHeight: 1.5, color: "var(--text-dim)", marginBottom: 4 }}>
        {t("gitea.desc")}
      </div>
      <Field label={t("gitea.baseUrl")}>
        <input
          className="vlx-input"
          type="text"
          value={baseUrl}
          onChange={(e) => setBaseUrl(e.target.value)}
          placeholder="https://git.example.com"
          disabled={busy}
          style={inputStyle}
        />
      </Field>
      <Field label={t("gitea.token")}>
        <input
          className="vlx-input"
          type="password"
          value={token}
          onChange={(e) => setToken(e.target.value)}
          placeholder={status?.hasToken ? t("gitea.tokenSet") : t("gitea.tokenPlaceholder")}
          disabled={busy}
          style={inputStyle}
        />
      </Field>
      <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 10 }}>
        <button
          className="vlx-btn"
          disabled={busy || !baseUrl.trim()}
          onClick={() => void test()}
        >
          {t("gitea.test")}
        </button>
        <button
          className="vlx-btn vlx-btn-primary"
          disabled={busy || !baseUrl.trim()}
          onClick={() => void save()}
        >
          {t("common.save")}
        </button>
      </div>
      {msg && (
        <div
          style={{
            marginTop: 8,
            fontSize: 11.5,
            lineHeight: 1.5,
            whiteSpace: "pre-wrap",
            color: msgOk ? "var(--text-dim)" : "var(--danger, #e05252)",
          }}
        >
          {msg}
        </div>
      )}
    </>
  );
}
