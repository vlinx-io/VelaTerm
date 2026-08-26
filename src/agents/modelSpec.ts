//! Per-agent model and reasoning-effort capabilities for the spawn dialog and launch-argument builder.
//!
//! Every agent CLI spells these differently — the flag name, the value shape, and whether a catalogue
//! can be listed at all — so one shared dropdown cannot serve them. This table is the single source of
//! truth: the dialog reads it to decide which control to render, and `executeSpawn` reads it to build
//! the right flag. Values were verified against the installed CLIs and official docs on 2026-06-09.

import type { AgentKind } from "../types";

/**
 * Where a model list comes from. The list is a suggestion, never a whitelist: the spawn dialog's model
 * box stays typeable for every source, so a dated identifier such as `claude-opus-4-6` or a locally
 * configured alias can always be passed through.
 * - `static`: the CLI ships a fixed set of aliases or presets, listed in `models`.
 * - `list`: the CLI can print its own catalogue, fetched through `agentListModels`.
 * - `free`: the CLI enumerates nothing, so there is nothing to suggest.
 */
export type ModelSource = "static" | "list" | "free";

/** Reasoning-effort control offered by an agent, or null when its CLI exposes none. */
export interface EffortSpec {
  /** Exact flag this CLI accepts, including dashes. */
  flag: string;
  /** Values the CLI documents; the dialog offers these and nothing else. */
  values: string[];
}

/** One agent's model and effort capabilities. */
export interface AgentModelSpec {
  /** Exact flag that selects a model, including dashes. */
  modelFlag: string;
  source: ModelSource;
  /** Fixed identifiers for `static` sources; unused otherwise. */
  models?: string[];
  /** Example value shown in the free-text field, guiding the expected shape. */
  placeholder?: string;
  effort: EffortSpec | null;
}

/**
 * Model and effort support per agent kind.
 *
 * Sources: `claude --help` plus the 2.1.245 binary's alias table; `codex` 0.146.0 built-in presets;
 * `opencode models`; `cursor-agent --list-models`; `pi --list-models`; `grok models` and
 * `grok --help`; Kimi Code, Kiro, Zoo/Roo, Antigravity, Cline, and Crush official documentation.
 */
export const AGENT_MODEL_SPECS: Record<AgentKind, AgentModelSpec> = {
  // Aliases resolve to the current model of each family. The `[1m]` variants request the 1M-token
  // context window and are separate selectable values, not a modifier.
  claude: {
    modelFlag: "--model",
    source: "static",
    models: [
      "fable",
      "fable[1m]",
      "opus",
      "opus[1m]",
      "opusplan",
      "opusplan[1m]",
      "sonnet",
      "sonnet[1m]",
      "haiku",
    ],
    effort: { flag: "--effort", values: ["low", "medium", "high", "xhigh", "max"] },
  },
  // Codex has no listing command; these are the presets compiled into the CLI. Reasoning effort is a
  // config key rather than a flag, so it is passed through the generic `-c key=value` override.
  codex: {
    modelFlag: "--model",
    source: "static",
    models: [
      "gpt-5.6-sol",
      "gpt-5.6-terra",
      "gpt-5.6-luna",
      "gpt-5.5",
      "gpt-5.4",
      "gpt-5.4-mini",
      "gpt-5.2",
    ],
    effort: null,
  },
  opencode: { modelFlag: "--model", source: "list", effort: null },
  // Copilot resolves models from the account's subscription and exposes no catalogue command; `auto`
  // lets it choose.
  copilot: {
    modelFlag: "--model",
    source: "free",
    placeholder: "auto",
    effort: null,
  },
  // Cursor accepts bracketed overrides on the model itself, e.g. `claude-opus-4-8[effort=high]`,
  // which is why it has no separate effort flag.
  cursor: { modelFlag: "--model", source: "list", effort: null },
  antigravity: {
    modelFlag: "--model",
    source: "list",
    effort: { flag: "--effort", values: ["low", "medium", "high"] },
  },
  // Cline's model set depends on the provider chosen with `--provider`, and it documents no listing
  // command. Its thinking control is graded rather than a toggle.
  cline: {
    modelFlag: "--model",
    source: "free",
    placeholder: "anthropic/claude-sonnet-4-6",
    effort: { flag: "--thinking", values: ["none", "low", "medium", "high", "xhigh"] },
  },
  pi: { modelFlag: "--model", source: "list", effort: null },
  // Crush configures reasoning in its crushrc script, not on the command line.
  crush: { modelFlag: "--model", source: "list", effort: null },
  // Kimi Code's managed provider exposes exactly these four; other providers are configured in
  // ~/.kimi/config.toml. Thinking is a boolean toggle, so it is not an effort scale.
  kimi: {
    modelFlag: "--model",
    source: "static",
    models: ["k3", "k3-256k", "kimi-for-coding", "kimi-for-coding-highspeed"],
    effort: null,
  },
  // Kiro only shows the effort levels the active model supports, so a chosen level may be rejected;
  // the CLI reports that itself rather than the dialog guessing.
  kiro: {
    modelFlag: "--model",
    source: "list",
    effort: { flag: "--effort", values: ["low", "medium", "high", "xhigh", "max"] },
  },
  grok: {
    modelFlag: "--model",
    source: "list",
    effort: {
      flag: "--reasoning-effort",
      values: ["none", "minimal", "low", "medium", "high", "xhigh", "max"],
    },
  },
  // Zoo/Roo pairs a `provider/model` identifier with a separate `--provider` flag and enumerates
  // neither.
  zoo: {
    modelFlag: "--model",
    source: "free",
    placeholder: "anthropic/claude-opus-4.6",
    effort: {
      flag: "--reasoning-effort",
      values: [
        "unspecified",
        "disabled",
        "none",
        "minimal",
        "low",
        "medium",
        "high",
        "xhigh",
      ],
    },
  },
};

/** The spec for an agent kind, or null for plain terminals and unknown kinds. */
export function modelSpec(kind: string): AgentModelSpec | null {
  return AGENT_MODEL_SPECS[kind as AgentKind] ?? null;
}

/**
 * Replace one flag's value in a launch-argument string, appending it when absent.
 *
 * Only the named flag is touched, so choosing an effort level never drops an inherited model. An
 * empty value removes the flag, which is how "Default" clears an inherited override.
 */
export function applyFlag(args: string, flag: string, value: string): string {
  const escaped = flag.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  // Values can carry brackets and slashes but never spaces, so a non-space run is the whole value.
  const stripped = args.replace(new RegExp(`${escaped}\\s+\\S+`, "g"), "").trim();
  return value ? `${stripped} ${flag} ${value}`.trim() : stripped;
}

/** Read a flag's current value out of a launch-argument string, or "" when it is absent. */
export function readFlag(args: string | null | undefined, flag: string): string {
  if (!args) return "";
  const escaped = flag.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const m = new RegExp(`${escaped}\\s+(\\S+)`).exec(args);
  return m ? m[1] : "";
}
