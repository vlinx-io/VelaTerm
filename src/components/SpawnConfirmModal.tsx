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
import { agentListModels } from "../ipc/commands";
import type { SpawnRequest } from "../ipc/events";
import { modelSpec, readFlag } from "../agents/modelSpec";
import { useTermStore } from "../store/termStore";
import Icons from "./Icons";
import Select, { SELECT_PANEL, selectRowStyle } from "./Select";

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
  { value: "kimi", label: "Kimi Code" },
  { value: "kiro", label: "Kiro" },
  { value: "grok", label: "Grok Build (Grok 4.5)" },
  { value: "zoo", label: "Zoo Code" },
  { value: "terminal", label: "Terminal" },
];

/**
 * Cache of listed model catalogues, keyed by agent kind and shared by every card in this window.
 *
 * Listing spawns the agent CLI and can take seconds, so a kind is asked at most once per app run.
 * A failed or empty listing caches an empty array too: retrying on every dropdown open would make the
 * dialog feel stuck for anyone whose CLI is not signed in.
 */
const modelCatalogCache = new Map<string, Promise<string[]>>();

function loadModelCatalog(kind: string): Promise<string[]> {
  const cached = modelCatalogCache.get(kind);
  if (cached) return cached;
  const p = agentListModels(kind).catch(() => [] as string[]);
  modelCatalogCache.set(kind, p);
  return p;
}

/**
 * Model field: a text box with a dropdown of the identifiers this agent is known to accept.
 *
 * The known list is a shortcut, not a whitelist. Agent CLIs take model identifiers this app has no
 * way to enumerate — a dated Claude id like `claude-opus-4-6`, a provider-prefixed name, a locally
 * configured alias — so the box stays editable and anything typed is passed through verbatim. An
 * empty box means "no --model flag", which is what Default does.
 */
function ModelCombo({
  value,
  onChange,
  options,
  placeholder,
  width = 190,
}: {
  value: string;
  onChange: (v: string) => void;
  options: string[];
  placeholder: string;
  width?: number;
}) {
  const [open, setOpen] = useState(false);
  const typed = value.trim().toLowerCase();
  // Typing narrows the list; a custom identifier simply matches nothing and the list stays out of the
  // way. The Default row appears only on an empty box, where clearing is not already one keystroke.
  const matches = typed
    ? options.filter((o) => o.toLowerCase().includes(typed))
    : options;
  const rows: { value: string; label: string }[] = [
    ...(typed ? [] : [{ value: "", label: placeholder }]),
    ...matches.map((m) => ({ value: m, label: m })),
  ];

  return (
    <div style={{ position: "relative", width }}>
      <input
        className="vlx-input"
        value={value}
        onChange={(e) => {
          onChange(e.target.value);
          setOpen(true);
        }}
        onFocus={() => setOpen(true)}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            // Swallow it here so the card's own handlers never see a dismissal meant for the list.
            e.stopPropagation();
            setOpen(false);
          }
        }}
        placeholder={placeholder}
        spellCheck={false}
        autoComplete="off"
        style={{
          width: "100%",
          height: 32,
          boxSizing: "border-box",
          paddingRight: 26,
          fontFamily: "var(--font-mono, monospace)",
          fontSize: 12.5,
        }}
      />
      <button
        type="button"
        aria-label={placeholder}
        onClick={() => setOpen((o) => !o)}
        style={{
          position: "absolute",
          top: 0,
          right: 0,
          width: 24,
          height: 32,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          background: "transparent",
          border: "none",
          padding: 0,
          cursor: "pointer",
        }}
      >
        <Icons.chevD size={12} style={{ color: "var(--text-muted)" }} />
      </button>

      {open && rows.length > 0 && (
        <>
          {/* Transparent backdrop closes the list on any outside click. */}
          <div
            style={{ position: "fixed", inset: 0, zIndex: 1140 }}
            onMouseDown={() => setOpen(false)}
          />
          <div style={{ ...SELECT_PANEL, left: 0, width }}>
            {rows.map((opt) => {
              const on = opt.value === value;
              return (
                <div
                  key={opt.value || "_default"}
                  // Mouse down fires before the input loses focus, so the choice is not lost to a blur.
                  onMouseDown={(e) => {
                    e.preventDefault();
                    onChange(opt.value);
                    setOpen(false);
                  }}
                  style={selectRowStyle(on, false, "md", true)}
                  onMouseEnter={(e) => {
                    if (!on) e.currentTarget.style.background = "var(--bg-hover)";
                  }}
                  onMouseLeave={(e) => {
                    if (!on) e.currentTarget.style.background = "transparent";
                  }}
                >
                  <span style={{ flex: 1 }}>{opt.label}</span>
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
  // Listed catalogue for the current kind, plus whether the listing call is still outstanding.
  const [catalog, setCatalog] = useState<string[]>([]);
  const [listing, setListing] = useState(false);

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
      parent?.kind === "kimi" ||
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
    const spec = modelSpec(kind);
    const inherited = parent && parent.kind === kind ? parent.agentArgs : "";
    const args = inherited || agentDefaults[kind]?.args || "";
    // Each agent spells these flags differently, so read back the flag this agent actually uses; a
    // Claude `--effort` sitting in another agent's arguments is not this agent's effort setting.
    setModel(spec ? readFlag(args, spec.modelFlag) : "");
    setEffort(spec?.effort ? readFlag(args, spec.effort.flag) : "");
    // Parent and defaults are derived from req and kind.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [req, kind]);

  // Fetch the catalogue for agents that can list one. The request is cached per kind, and a stale
  // guard drops a slow answer once the user has already switched to a different agent.
  useEffect(() => {
    const spec = modelSpec(kind);
    if (!req || spec?.source !== "list") {
      setCatalog([]);
      setListing(false);
      return;
    }
    let live = true;
    setCatalog([]);
    setListing(true);
    void loadModelCatalog(kind).then((models) => {
      if (!live) return;
      setCatalog(models);
      setListing(false);
    });
    return () => {
      live = false;
    };
  }, [req, kind]);

  // Suspend native browser views while the card is visible so they cannot cover it.
  useSuspendNativeViews(Boolean(req));

  if (!req) return null;

  const canLaunch = prompt.trim().length > 0;
  const remaining = queue.length - 1;
  const spec = modelSpec(kind);
  // Every agent CLI takes a model; only plain terminals have nothing to choose.
  const showModel = spec !== null;
  const listedModels = spec?.source === "static" ? (spec.models ?? []) : catalog;
  const effortOptions = spec?.effort
    ? [
        { value: "", label: t("spawn.modelDefault") },
        ...spec.effort.values.map((v) => ({ value: v, label: v })),
      ]
    : [];

  const launch = () => {
    if (!canLaunch) return;
    void confirmSpawn({
      parentSessionId: req.parentSessionId,
      prompt,
      kind,
      worktree,
      // Agents without these selectors never receive an override: the fields are hidden, so any value
      // still held there is a leftover from another agent the user cannot see or clear.
      model: showModel ? model.trim() || null : null,
      effort: spec?.effort ? effort || null : null,
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
          <div style={{ display: "block" }}>
            <div
              style={{
                fontSize: 11,
                color: "var(--text-muted)",
                marginBottom: 4,
              }}
            >
              {t("spawn.agentLabel")}
            </div>
            <Select value={kind} onChange={setKind} options={KIND_OPTIONS} width={150} mono />
          </div>

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

        {/* Model and effort controls. The model box is always typeable and offers whatever the agent is
            known to accept as a dropdown, so an identifier this app cannot enumerate can still be
            entered. Agents whose CLI has no effort flag get no effort selector. */}
        {showModel && spec && (
          <div style={{ display: "flex", gap: 14, alignItems: "flex-end" }}>
            <div style={{ display: "block" }}>
              <div
                style={{
                  fontSize: 11,
                  color: "var(--text-muted)",
                  marginBottom: 4,
                }}
              >
                {t("spawn.modelLabel")}
              </div>
              <ModelCombo
                value={model}
                onChange={setModel}
                options={listedModels}
                placeholder={
                  spec.source === "free"
                    ? (spec.placeholder ?? t("spawn.modelDefault"))
                    : t("spawn.modelDefault")
                }
                width={190}
              />
              {/* Listing runs the agent CLI, so say what is happening instead of showing a dropdown
                  that briefly holds nothing but Default. */}
              {spec.source === "list" && (listing || catalog.length === 0) && (
                <div
                  style={{
                    fontSize: 10.5,
                    color: "var(--text-muted)",
                    marginTop: 4,
                    maxWidth: 190,
                  }}
                >
                  {listing ? t("spawn.modelLoading") : t("spawn.modelListUnavailable")}
                </div>
              )}
            </div>

            {spec.effort && (
              <div style={{ display: "block" }}>
                <div
                  style={{
                    fontSize: 11,
                    color: "var(--text-muted)",
                    marginBottom: 4,
                  }}
                >
                  {t("spawn.effortLabel")}
                </div>
                <Select
                  value={effort}
                  onChange={setEffort}
                  options={effortOptions}
                  width={120}
                  mono
                />
              </div>
            )}
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
