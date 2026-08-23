//! Child-session spawn confirmation card. When pre-spawn confirmation is enabled, requests from
//! vspawn or the `/vspawn` skill let the user review and edit the prompt, agent type, model,
//! effort, and worktree choice before starting, or cancel without creating a session. Multiple
//! requests queue in arrival order.
//!
//! The interaction follows Claude Desktop's spawn prompt: a nonmodal upper-right card that does not
//! take focus or block terminal input. Outside clicks and Escape do not dismiss it, preventing
//! accidental loss; only the card's Cancel and Start actions resolve the request.
//!
//! State comes from the store's pendingSpawns queue; like DirectoryPickerModal, it mounts at the App root.

import { useEffect, useState } from "react";
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

/** Model options available in the model selector. Empty string means inherit from parent/defaults. */
const MODEL_OPTIONS: { value: string; label: string }[] = [
  { value: "", label: "Default" },
  { value: "sonnet", label: "Sonnet" },
  { value: "opus", label: "Opus" },
  { value: "haiku", label: "Haiku" },
];

/** Effort options. Empty string means inherit from parent/defaults. */
const EFFORT_OPTIONS: { value: string; label: string }[] = [
  { value: "", label: "Default" },
  { value: "low", label: "Low" },
  { value: "medium", label: "Medium" },
  { value: "high", label: "High" },
  { value: "max", label: "Max" },
];

/** Agent kinds that accept --model / --effort flags. */
const SUPPORTS_MODEL_EFFORT = new Set<string>(["claude"]);

/** Extract a flag value from an agentArgs string, e.g. parseFlag("--model opus --effort high", "model") → "opus". */
function parseFlag(args: string | null | undefined, flag: string): string {
  if (!args) return "";
  const re = new RegExp(`--${flag}\\s+(\\S+)`);
  const m = re.exec(args);
  return m ? m[1] : "";
}

/**
 * Generic dropdown for the spawn card, reused by agent kind, model, and effort selectors.
 * Native `<select>` is avoided because macOS renders a system arrow and highlighted border that
 * conflict with the dark form.
 */
function SpawnSelect<T extends string>({
  value,
  onChange,
  options,
  width = 120,
}: {
  value: T;
  onChange: (v: T) => void;
  options: { value: T; label: string }[];
  width?: number;
}) {
  const [open, setOpen] = useState(false);
  const current = options.find((o) => o.value === value)?.label ?? value;

  return (
    <div style={{ position: "relative" }}>
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
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
            textAlign: "left",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
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
                  key={opt.value || "_default"}
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

export function SpawnConfirmModal() {
  const t = useT();
  const queue = useTermStore((s) => s.pendingSpawns);
  const sessions = useTermStore((s) => s.sessions);
  const agentDefaults = useTermStore((s) => s.agentDefaults);
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
    const resolvedKind = (req.kind ?? null) || fallback;
    setPrompt(req.prompt);
    setKind(resolvedKind);
    // Worktrees default on, matching backend and legacy behavior; only explicit false disables them.
    setWorktree(req.worktree !== false);
    // Depend only on req because parent is derived from it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [req]);

  // Model and effort belong to the agent that will actually run, so re-derive them whenever the kind
  // changes. Inheriting the parent's arguments only makes sense for the same agent: carrying a Claude
  // model name onto a Codex command line launches the child with a model it does not have.
  useEffect(() => {
    if (!req) return;
    const inherited = parent && parent.kind === kind ? parent.agentArgs : "";
    const args = inherited || agentDefaults[kind]?.args || "";
    setModel(parseFlag(args, "model"));
    setEffort(parseFlag(args, "effort"));
    // Parent and defaults are derived from req and kind.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [req, kind]);

  // Suspend native browser views while the card is visible so they cannot cover it.
  useSuspendNativeViews(Boolean(req));

  if (!req) return null;

  const canLaunch = prompt.trim().length > 0;
  const remaining = queue.length - 1;
  const showModelEffort = SUPPORTS_MODEL_EFFORT.has(kind);

  const launch = () => {
    if (!canLaunch) return;
    void confirmSpawn({
      parentSessionId: req.parentSessionId,
      prompt,
      kind,
      worktree,
      // Agents without these selectors never receive an override: the fields are hidden, so any value
      // still held there is a leftover from another agent the user cannot see or clear.
      model: showModelEffort ? model || null : null,
      effort: showModelEffort ? effort || null : null,
    });
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
            <SpawnSelect value={kind} onChange={setKind} options={KIND_OPTIONS} width={150} />
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

        {/* Model and effort selectors, shown only for agent kinds that support them. */}
        {showModelEffort && (
          <div style={{ display: "flex", gap: 14, alignItems: "flex-end" }}>
            <label style={{ display: "block" }}>
              <div
                style={{
                  fontSize: 11,
                  color: "var(--text-muted)",
                  marginBottom: 4,
                }}
              >
                {t("spawn.modelLabel")}
              </div>
              <SpawnSelect value={model} onChange={setModel} options={MODEL_OPTIONS} width={120} />
            </label>

            <label style={{ display: "block" }}>
              <div
                style={{
                  fontSize: 11,
                  color: "var(--text-muted)",
                  marginBottom: 4,
                }}
              >
                {t("spawn.effortLabel")}
              </div>
              <SpawnSelect value={effort} onChange={setEffort} options={EFFORT_OPTIONS} width={120} />
            </label>
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
