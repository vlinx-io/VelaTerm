//! Child-session spawn confirmation card. When pre-spawn confirmation is enabled, requests from
//! vspawn or the `/vspawn` skill let the user review and edit the prompt, agent type, and worktree
//! choice before starting, or cancel without creating a session. Multiple requests queue in arrival order.
//!
//! The interaction follows Claude Desktop's spawn prompt: a nonmodal upper-right card that does not
//! take focus or block terminal input. Outside clicks and Escape do not dismiss it, preventing
//! accidental loss; only the card's Cancel and Start actions resolve the request.
//!
//! State comes from the store's pendingSpawns queue; like DirectoryPickerModal, it mounts at the App root.

import { useEffect, useState, type ReactNode } from "react";
import { useT } from "../i18n";
import { useSuspendNativeViews } from "../hooks/nativeViewSuspend";
import type { SpawnRequest } from "../ipc/events";
import { useTermStore } from "../store/termStore";
import Icons from "./Icons";

/** Agent types available in the selector, matching non-null SpawnRequest.kind values. */
type SpawnKind = NonNullable<SpawnRequest["kind"]>;
const KIND_OPTIONS: { value: SpawnKind; label: string }[] = [
  { value: "claude", label: "Claude" },
  { value: "codex", label: "Codex" },
  { value: "opencode", label: "OpenCode" },
  { value: "copilot", label: "Copilot" },
  { value: "cursor", label: "Cursor" },
  { value: "antigravity", label: "Antigravity" },
  { value: "cline", label: "Cline" },
  { value: "pi", label: "Pi" },
  { value: "crush", label: "Crush" },
  { value: "kiro", label: "Kiro" },
  { value: "grok", label: "Grok Build (Grok 4.5)" },
  { value: "zoo", label: "Zoo Code" },
  { value: "terminal", label: "Terminal" },
];

/** Agents whose model/effort settings translate to launch flags; see inject::model_effort_flags.
 *  The card hides both fields for other kinds because their values would be ignored at launch. */
export const LAUNCH_CONFIG_KINDS: SpawnKind[] = ["claude", "codex"];

/** Curated per-agent choices. `Other…` in the dropdown reveals a free-text input as the escape
 *  hatch for models newer than this build. */
export const MODEL_OPTIONS: Record<string, string[]> = {
  claude: ["fable", "opus", "sonnet", "haiku"],
  codex: ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"],
};
export const EFFORT_OPTIONS: Record<string, string[]> = {
  claude: ["low", "medium", "high"],
  codex: ["low", "medium", "high", "xhigh", "max"],
};

/** Sentinel dropdown value that switches a launch-value field into free-text mode. */
export const OTHER_VALUE = "__other__";

/**
 * Generic dropdown matching the settings LangSelect. Native `<select>` is avoided because macOS
 * renders a system arrow and highlighted border that conflict with the dark form.
 */
export function ValueSelect({
  value,
  options,
  onChange,
  width = 150,
  ariaLabel,
}: {
  value: string;
  options: { value: string; label: string; icon?: ReactNode }[];
  onChange: (v: string) => void;
  width?: number | string;
  ariaLabel?: string;
}) {
  const [open, setOpen] = useState(false);
  const currentOption = options.find((o) => o.value === value);
  const current = currentOption?.label ?? value;

  return (
    <div style={{ position: "relative" }}>
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        aria-label={ariaLabel}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          width,
          height: 32,
          padding: "0 9px",
          background: "var(--bg-app)",
          color: "var(--text-primary)",
          border: `1px solid ${open ? "var(--accent)" : "var(--border)"}`,
          borderRadius: 5,
          fontFamily: "var(--font-mono)",
          fontSize: 12.5,
          cursor: "pointer",
        }}
      >
        <span
          style={{
            flex: 1,
            display: "inline-flex",
            alignItems: "center",
            gap: 6,
            textAlign: "left",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {currentOption?.icon && (
            <span style={{ display: "inline-flex", color: "var(--accent)", flex: "none" }}>
              {currentOption.icon}
            </span>
          )}
          {current}
        </span>
        <Icons.chevD
          size={12}
          style={{ color: "var(--text-muted)", flex: "none" }}
        />
      </button>

      {open && (
        <>
          {/* Transparent backdrop closes the dropdown on any outside click. */}
          <div
            style={{ position: "fixed", inset: 0, zIndex: 1140 }}
            onMouseDown={() => setOpen(false)}
          />
          <div
            style={{
              position: "absolute",
              top: "calc(100% + 4px)",
              left: 0,
              zIndex: 1150,
              width,
              maxHeight: 260,
              overflowY: "auto",
              padding: 4,
              background: "var(--bg-panel)",
              border: "1px solid var(--border-strong)",
              borderRadius: 8,
              boxShadow: "0 10px 30px rgba(0,0,0,0.5)",
            }}
          >
            {options.map((opt) => {
              const on = opt.value === value;
              return (
                <div
                  key={opt.value}
                  onClick={() => {
                    onChange(opt.value);
                    setOpen(false);
                  }}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 6,
                    padding: "6px 8px",
                    borderRadius: 6,
                    cursor: "pointer",
                    fontSize: 12.5,
                    color: on ? "var(--accent)" : "var(--text-primary)",
                    background: on ? "var(--accent-soft)" : "transparent",
                  }}
                  onMouseEnter={(e) => {
                    if (!on)
                      e.currentTarget.style.background = "var(--bg-elevated)";
                  }}
                  onMouseLeave={(e) => {
                    if (!on) e.currentTarget.style.background = "transparent";
                  }}
                >
                  {opt.icon && (
                    <span style={{ display: "inline-flex", color: "var(--accent)", flex: "none" }}>
                      {opt.icon}
                    </span>
                  )}
                  <span style={{ flex: 1 }}>{opt.label}</span>
                  {on && <Icons.check size={12} style={{ flex: "none" }} />}
                </div>
              );
            })}
          </div>
        </>
      )}
    </div>
  );
}

function KindSelect({
  value,
  onChange,
}: {
  value: SpawnKind;
  onChange: (v: SpawnKind) => void;
}) {
  return (
    <ValueSelect
      value={value}
      options={KIND_OPTIONS}
      onChange={(v) => onChange(v as SpawnKind)}
    />
  );
}

export function SpawnConfirmModal() {
  const t = useT();
  const queue = useTermStore((s) => s.pendingSpawns);
  const sessions = useTermStore((s) => s.sessions);
  const confirmSpawn = useTermStore((s) => s.confirmSpawn);
  const cancelSpawn = useTermStore((s) => s.cancelSpawn);

  // The queue head is the active request; an empty queue renders no card.
  const req = queue.length > 0 ? queue[0] : null;
  const parent = req
    ? sessions.find((s) => s.id === req.parentSessionId)
    : undefined;

  const [prompt, setPrompt] = useState("");
  const [kind, setKind] = useState<SpawnKind>("claude");
  const [worktree, setWorktree] = useState(true);
  const [model, setModel] = useState("");
  const [effort, setEffort] = useState("");
  // Free-text mode for values outside the curated per-agent lists.
  const [modelOther, setModelOther] = useState(false);
  const [effortOther, setEffortOther] = useState(false);

  // Reset fields when the queue head changes after enqueue, confirmation, or cancellation. Each
  // queued request is a new object, so reference changes reliably trigger the reset.
  useEffect(() => {
    if (!req) return;
    // Default to the parent agent type; use Claude when the parent is absent or is not an agent.
    const fallback: SpawnKind =
      parent?.kind === "codex" ||
      parent?.kind === "opencode" ||
      parent?.kind === "copilot" ||
      parent?.kind === "cursor" ||
      parent?.kind === "antigravity" ||
      parent?.kind === "cline" ||
      parent?.kind === "pi" ||
      parent?.kind === "crush" ||
      parent?.kind === "kiro" ||
      parent?.kind === "grok" ||
      parent?.kind === "zoo"
        ? parent.kind
        : "claude";
    setPrompt(req.prompt);
    const k = (req.kind ?? null) || fallback;
    setKind(k);
    // Worktrees default on, matching backend and legacy behavior; only explicit false disables them.
    setWorktree(req.worktree !== false);
    // Empty model/effort keep the agent defaults; unlisted values open in free-text mode.
    const m = req.model ?? "";
    const e = req.effort ?? "";
    setModel(m);
    setEffort(e);
    setModelOther(!!m && !(MODEL_OPTIONS[k] ?? []).includes(m));
    setEffortOther(!!e && !(EFFORT_OPTIONS[k] ?? []).includes(e));
    // Depend only on req because parent is derived from it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [req]);

  // Suspend native browser views while the card is visible so they cannot cover it.
  useSuspendNativeViews(Boolean(req));

  if (!req) return null;

  const canLaunch = prompt.trim().length > 0;
  const remaining = queue.length - 1;

  // Model/effort only apply to agents whose launch flags support them; drop silently elsewhere.
  const hasLaunchConfig = LAUNCH_CONFIG_KINDS.includes(kind);

  const launch = () => {
    if (!canLaunch) return;
    void confirmSpawn({
      parentSessionId: req.parentSessionId,
      prompt,
      kind,
      worktree,
      model: hasLaunchConfig ? model.trim() || null : null,
      effort: hasLaunchConfig ? effort.trim() || null : null,
      name: req.name ?? null,
      agentArgs: req.agentArgs ?? null,
      permissionMode: req.permissionMode ?? null,
      requestId: req.requestId ?? null,
    });
  };

  // Switching agents keeps chosen values but re-evaluates whether they need free-text mode.
  const changeKind = (k: SpawnKind) => {
    setKind(k);
    setModelOther(!!model && !(MODEL_OPTIONS[k] ?? []).includes(model));
    setEffortOther(!!effort && !(EFFORT_OPTIONS[k] ?? []).includes(effort));
  };

  return (
    <div
      role="dialog"
      aria-label={t("spawn.title")}
      style={{
        position: "fixed",
        top: 46, // Leave 8 px below the 38 px title bar.
        right: 16,
        zIndex: 1100,
        width: 400,
        maxWidth: "calc(100vw - 32px)",
        background: "var(--bg-panel)",
        border: "1px solid var(--border-strong)",
        borderRadius: 10,
        padding: 18,
        boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
      }}
      onKeyDown={(e) => {
        // The card does not claim focus. Once focused, such as by clicking the textarea, Cmd/Ctrl+Enter starts.
        // Escape and outside clicks remain unbound so accidental actions cannot dismiss the request.
        if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) launch();
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "baseline",
          justifyContent: "space-between",
          gap: 8,
          marginBottom: 14,
        }}
      >
        <div
          style={{
            fontSize: 14,
            fontWeight: 600,
            color: "var(--text-primary)",
          }}
        >
          {t("spawn.title")}
        </div>
        {remaining > 0 && (
          <div style={{ fontSize: 11, color: "var(--text-muted)" }}>
            {t("spawn.remaining", remaining)}
          </div>
        )}
      </div>

      {parent && (
        <div
          style={{
            fontSize: 11,
            color: "var(--text-muted)",
            marginBottom: 12,
          }}
        >
          {t("spawn.fromSession")}: {parent.name}
        </div>
      )}

      {req.launchWarnings && req.launchWarnings.length > 0 && (
        <div
          role="alert"
          style={{
            marginBottom: 12,
            padding: "8px 10px",
            border: "1px solid var(--status-asking)",
            borderRadius: 5,
            color: "var(--text-primary)",
            fontSize: 11,
            lineHeight: 1.45,
          }}
        >
          <div style={{ color: "var(--status-asking)", fontWeight: 600, marginBottom: 3 }}>
            {t("status.error")}
          </div>
          {req.launchWarnings.map((warning) => (
            <div key={warning}>{warning}</div>
          ))}
        </div>
      )}

      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        {/* Editable multiline prompt without autoFocus, preserving terminal focus when the card appears. */}
        <label style={{ display: "block" }}>
          <div
            style={{
              fontSize: 11,
              color: "var(--text-muted)",
              marginBottom: 4,
            }}
          >
            {t("spawn.promptLabel")}
          </div>
          <textarea
            className="vlx-input"
            rows={6}
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            style={{
              width: "100%",
              boxSizing: "border-box",
              resize: "vertical",
              minHeight: 110,
              fontFamily: "var(--font-mono, monospace)",
              fontSize: 12.5,
              lineHeight: 1.5,
            }}
          />
        </label>

        {/* Agent type and worktree toggle on one row. */}
        <div style={{ display: "flex", gap: 14, alignItems: "flex-end" }}>
          <label style={{ display: "block" }}>
            <div
              style={{
                fontSize: 11,
                color: "var(--text-muted)",
                marginBottom: 4,
              }}
            >
              {t("spawn.agentLabel")}
            </div>
            <KindSelect value={kind} onChange={changeKind} />
          </label>

          <label
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              fontSize: 13,
              color: "var(--text-primary)",
              cursor: "pointer",
              paddingBottom: 5,
            }}
          >
            <input
              type="checkbox"
              checked={worktree}
              onChange={(e) => setWorktree(e.target.checked)}
            />
            {t("spawn.worktreeLabel")}
          </label>
        </div>

        {/* Model and effort dropdowns; "default" keeps the agent defaults and "Other..." reveals a
            free-text input. Hidden entirely for agents whose launch flags ignore these values. */}
        {hasLaunchConfig && (
          <div style={{ display: "flex", gap: 14 }}>
            <div style={{ flex: 1 }}>
              <div
                style={{
                  fontSize: 11,
                  color: "var(--text-muted)",
                  marginBottom: 4,
                }}
              >
                {t("spawn.modelLabel")}
              </div>
              <ValueSelect
                width="100%"
                value={modelOther ? OTHER_VALUE : model}
                options={[
                  { value: "", label: t("spawn.optionDefault") },
                  ...(MODEL_OPTIONS[kind] ?? []).map((m) => ({
                    value: m,
                    label: m,
                  })),
                  { value: OTHER_VALUE, label: t("spawn.optionOther") },
                ]}
                onChange={(v) => {
                  if (v === OTHER_VALUE) {
                    setModelOther(true);
                    setModel("");
                  } else {
                    setModelOther(false);
                    setModel(v);
                  }
                }}
              />
              {modelOther && (
                <input
                  className="vlx-input"
                  value={model}
                  onChange={(e) => setModel(e.target.value)}
                  placeholder={t("spawn.modelLabel")}
                  style={{
                    width: "100%",
                    boxSizing: "border-box",
                    height: 32,
                    marginTop: 6,
                    fontFamily: "var(--font-mono)",
                    fontSize: 12.5,
                  }}
                />
              )}
            </div>
            <div style={{ flex: 1 }}>
              <div
                style={{
                  fontSize: 11,
                  color: "var(--text-muted)",
                  marginBottom: 4,
                }}
              >
                {t("spawn.effortLabel")}
              </div>
              <ValueSelect
                width="100%"
                value={effortOther ? OTHER_VALUE : effort}
                options={[
                  { value: "", label: t("spawn.optionDefault") },
                  ...(EFFORT_OPTIONS[kind] ?? []).map((e) => ({
                    value: e,
                    label: e,
                  })),
                  { value: OTHER_VALUE, label: t("spawn.optionOther") },
                ]}
                onChange={(v) => {
                  if (v === OTHER_VALUE) {
                    setEffortOther(true);
                    setEffort("");
                  } else {
                    setEffortOther(false);
                    setEffort(v);
                  }
                }}
              />
              {effortOther && (
                <input
                  className="vlx-input"
                  value={effort}
                  onChange={(e) => setEffort(e.target.value)}
                  placeholder={t("spawn.effortLabel")}
                  style={{
                    width: "100%",
                    boxSizing: "border-box",
                    height: 32,
                    marginTop: 6,
                    fontFamily: "var(--font-mono)",
                    fontSize: 12.5,
                  }}
                />
              )}
            </div>
          </div>
        )}
      </div>

      <div
        style={{
          display: "flex",
          justifyContent: "flex-end",
          gap: 8,
          marginTop: 18,
        }}
      >
        <button className="vlx-btn" onClick={cancelSpawn}>
          {t("common.cancel")}
        </button>
        <button
          className="vlx-btn vlx-btn-primary"
          onClick={launch}
          disabled={!canLaunch}
        >
          {t("spawn.launch")}
        </button>
      </div>
    </div>
  );
}
