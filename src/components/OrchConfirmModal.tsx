//! Confirmation dialog for a `/vorch` orchestration: several agents proposed at once, reviewed one tab
//! at a time before any of them starts.
//!
//! Unlike SpawnConfirmModal — a small non-modal card for a single request — this takes a real backdrop
//! and the user's attention. It has to: the decomposition was written by a model, each entry starts a
//! real agent process, and this dialog is the only place any of it can be corrected. The prompt editor
//! is therefore the largest thing on screen.
//!
//! Per-agent settings default to "follow shared" and only override when touched, so the common case is
//! one setting in one place. A changed tab is marked, so a user who adjusted one entry ten tabs ago can
//! still see that they did.

import { useEffect, useMemo, useState } from "react";
import { useT } from "../i18n";
import type { OrchAgentSpec } from "../ipc/events";
import { useTermStore } from "../store/termStore";
import { Backdrop } from "./Backdrop";
import Icons from "./Icons";

/** Agent types that can be orchestrated. Terminal is absent: an entry has a task, so it needs an agent. */
const KINDS = [
  "claude",
  "codex",
  "opencode",
  "copilot",
  "cursor",
  "antigravity",
  "cline",
  "pi",
  "crush",
  "kiro",
  "grok",
  "zoo",
] as const;

/**
 * Effort levels a given agent publishes as a closed set.
 *
 * Only Claude and Copilot document theirs; every other CLI takes its levels from a model catalog that
 * changes server-side, so offering a fixed menu there would produce combinations the agent rejects.
 * Those get a free-text field instead, and an empty value simply passes no effort at all.
 */
const EFFORT_CHOICES: Record<string, string[]> = {
  claude: ["low", "medium", "high", "xhigh", "max"],
  copilot: ["none", "low", "medium", "high", "xhigh", "max"],
};

type WorktreeMode = "none" | "shared" | "each";

/** Small labelled field wrapper, so the three settings rows line up without repeating layout code. */
function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label style={{ display: "flex", flexDirection: "column", gap: 4, minWidth: 0, flex: 1 }}>
      <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{label}</span>
      {children}
    </label>
  );
}

export function OrchConfirmModal() {
  const t = useT();
  const req = useTermStore((s) => s.pendingOrch);
  const confirmOrch = useTermStore((s) => s.confirmOrch);
  const cancelOrch = useTermStore((s) => s.cancelOrch);

  const [title, setTitle] = useState("");
  const [mode, setMode] = useState<WorktreeMode>("each");
  const [kind, setKind] = useState<string>("claude");
  const [model, setModel] = useState("");
  const [effort, setEffort] = useState("");
  const [agents, setAgents] = useState<OrchAgentSpec[]>([]);
  const [active, setActive] = useState(0);

  // Reset every time a proposal arrives; a stale draft from a previous run must never leak into a new one.
  useEffect(() => {
    if (!req) return;
    setTitle(req.title);
    setMode((req.worktreeMode as WorktreeMode) ?? "each");
    setKind(req.defaults?.kind ?? "claude");
    setModel(req.defaults?.model ?? "");
    setEffort(req.defaults?.effort ?? "");
    setAgents(req.agents.map((a, i) => ({ ...a, idx: i })));
    setActive(0);
  }, [req]);

  const current = agents[active];
  const sharedEfforts = EFFORT_CHOICES[kind];

  /** Whether this entry overrides anything, which is what the tab marker reports. */
  const overridden = useMemo(
    () =>
      agents.map(
        (a) =>
          a.kind != null || a.model != null || a.effort != null || a.worktree != null,
      ),
    [agents],
  );

  if (!req || !current) return null;

  const patch = (changes: Partial<OrchAgentSpec>) =>
    setAgents((list) => list.map((a, i) => (i === active ? { ...a, ...changes } : a)));

  const remove = () => {
    setAgents((list) => list.filter((_, i) => i !== active));
    setActive((i) => Math.max(0, i - 1));
  };

  const start = () => {
    void confirmOrch({
      ...req,
      title: title.trim() || req.title,
      worktreeMode: mode,
      defaults: {
        kind: kind as OrchAgentSpec["kind"],
        model: model.trim() || null,
        effort: effort.trim() || null,
      },
      agents,
    });
  };

  const agentEfforts = EFFORT_CHOICES[current.kind ?? kind];

  // A click outside must not cancel: cancelling records every entry as dropped and the proposing
  // session has already spent a turn composing it. Only the Cancel button ends the run.
  const keepOpen = () => {};

  return (
    <Backdrop onClose={keepOpen}>
      <div
        style={{
          width: 620,
          maxHeight: "84vh",
          display: "flex",
          flexDirection: "column",
          background: "var(--bg-panel)",
          border: "1px solid var(--border)",
          borderRadius: 10,
          boxShadow: "0 10px 40px rgba(0,0,0,0.5)",
          overflow: "hidden",
        }}
      >
        <div style={{ padding: "14px 18px 10px", borderBottom: "1px solid var(--border)" }}>
          <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 8 }}>{t("orch.title")}</div>
          <input
            className="vlx-input"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            style={{ width: "100%" }}
          />
        </div>

        <div
          style={{
            padding: "10px 18px",
            borderBottom: "1px solid var(--border)",
            display: "flex",
            flexDirection: "column",
            gap: 8,
          }}
        >
          <div style={{ fontSize: 11, color: "var(--text-muted)" }}>{t("orch.sharedSettings")}</div>
          <div style={{ display: "flex", gap: 10 }}>
            <Field label={t("orch.agentLabel")}>
              <select className="vlx-input" value={kind} onChange={(e) => setKind(e.target.value)}>
                {KINDS.map((k) => (
                  <option key={k} value={k}>
                    {k}
                  </option>
                ))}
              </select>
            </Field>
            <Field label={t("orch.modelLabel")}>
              <input
                className="vlx-input"
                value={model}
                placeholder={t("orch.modelPlaceholder")}
                onChange={(e) => setModel(e.target.value)}
              />
            </Field>
            <Field label={t("orch.effortLabel")}>
              {sharedEfforts ? (
                <select
                  className="vlx-input"
                  value={effort}
                  onChange={(e) => setEffort(e.target.value)}
                >
                  <option value="">{t("orch.effortPlaceholder")}</option>
                  {sharedEfforts.map((e) => (
                    <option key={e} value={e}>
                      {e}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  className="vlx-input"
                  value={effort}
                  placeholder={t("orch.effortPlaceholder")}
                  onChange={(e) => setEffort(e.target.value)}
                />
              )}
            </Field>
          </div>
          <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
            <span style={{ fontSize: 11, color: "var(--text-muted)", marginRight: 4 }}>
              {t("orch.worktreeLabel")}
            </span>
            {(
              [
                ["none", t("orch.worktreeNone")],
                ["shared", t("orch.worktreeShared")],
                ["each", t("orch.worktreeEach")],
              ] as [WorktreeMode, string][]
            ).map(([value, label]) => (
              <button
                key={value}
                className={mode === value ? "vlx-btn vlx-btn-primary" : "vlx-btn"}
                style={{ fontSize: 11, padding: "2px 8px" }}
                onClick={() => setMode(value)}
              >
                {label}
              </button>
            ))}
          </div>
        </div>

        <div
          style={{
            display: "flex",
            gap: 4,
            padding: "8px 18px 0",
            overflowX: "auto",
            borderBottom: "1px solid var(--border)",
          }}
        >
          {agents.map((a, i) => (
            <button
              key={i}
              className={i === active ? "vlx-btn vlx-btn-primary" : "vlx-btn"}
              style={{ fontSize: 11, padding: "3px 10px", whiteSpace: "nowrap", marginBottom: 8 }}
              onClick={() => setActive(i)}
              title={overridden[i] ? t("orch.overridden") : undefined}
            >
              {a.name || `#${i + 1}`}
              {overridden[i] ? " ·" : ""}
            </button>
          ))}
        </div>

        <div
          style={{
            padding: "12px 18px",
            display: "flex",
            flexDirection: "column",
            gap: 10,
            overflowY: "auto",
            flex: 1,
          }}
        >
          <div style={{ display: "flex", gap: 10, alignItems: "flex-end" }}>
            <Field label={t("orch.nameLabel")}>
              <input
                className="vlx-input"
                value={current.name}
                onChange={(e) => patch({ name: e.target.value })}
              />
            </Field>
            <button
              className="vlx-btn"
              style={{ fontSize: 11 }}
              onClick={remove}
              disabled={agents.length <= 1}
            >
              <Icons.trash />
              {t("orch.remove")}
            </button>
          </div>

          <label style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <span style={{ fontSize: 11, color: "var(--text-muted)" }}>{t("orch.promptLabel")}</span>
            <textarea
              className="vlx-input"
              value={current.prompt}
              rows={9}
              onChange={(e) => patch({ prompt: e.target.value })}
              style={{ resize: "vertical", fontFamily: "var(--font-mono)", fontSize: 12 }}
            />
          </label>

          <div style={{ display: "flex", gap: 10 }}>
            <Field label={t("orch.agentLabel")}>
              <select
                className="vlx-input"
                value={current.kind ?? ""}
                onChange={(e) =>
                  patch({ kind: (e.target.value || null) as OrchAgentSpec["kind"] })
                }
              >
                <option value="">{t("orch.follow")}</option>
                {KINDS.map((k) => (
                  <option key={k} value={k}>
                    {k}
                  </option>
                ))}
              </select>
            </Field>
            <Field label={t("orch.modelLabel")}>
              <input
                className="vlx-input"
                value={current.model ?? ""}
                placeholder={t("orch.follow")}
                onChange={(e) => patch({ model: e.target.value || null })}
              />
            </Field>
            <Field label={t("orch.effortLabel")}>
              {agentEfforts ? (
                <select
                  className="vlx-input"
                  value={current.effort ?? ""}
                  onChange={(e) => patch({ effort: e.target.value || null })}
                >
                  <option value="">{t("orch.follow")}</option>
                  {agentEfforts.map((e) => (
                    <option key={e} value={e}>
                      {e}
                    </option>
                  ))}
                </select>
              ) : (
                <input
                  className="vlx-input"
                  value={current.effort ?? ""}
                  placeholder={t("orch.follow")}
                  onChange={(e) => patch({ effort: e.target.value || null })}
                />
              )}
            </Field>
            <Field label={t("orch.worktreeLabel")}>
              <select
                className="vlx-input"
                value={current.worktree == null ? "" : current.worktree ? "yes" : "no"}
                onChange={(e) =>
                  patch({ worktree: e.target.value === "" ? null : e.target.value === "yes" })
                }
              >
                <option value="">{t("orch.follow")}</option>
                <option value="yes">{t("orch.worktreeEach")}</option>
                <option value="no">{t("orch.worktreeNone")}</option>
              </select>
            </Field>
          </div>
        </div>

        <div
          style={{
            padding: "10px 18px",
            borderTop: "1px solid var(--border)",
            display: "flex",
            justifyContent: "flex-end",
            gap: 8,
          }}
        >
          <button className="vlx-btn" onClick={cancelOrch}>
            {t("common.cancel")}
          </button>
          <button className="vlx-btn vlx-btn-primary" onClick={start} disabled={agents.length === 0}>
            {t("orch.launch", agents.length)}
          </button>
        </div>
      </div>
    </Backdrop>
  );
}
