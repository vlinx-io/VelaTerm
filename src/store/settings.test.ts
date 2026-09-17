//! Coverage for the per-agent default view: the built-in fallback, the saved per-agent choice, and the
//! migration from the retired app-wide setting. Every chat-capable agent shares the conversation fallback;
//! saved per-agent choices can still override it.

import { beforeEach, describe, expect, it, vi } from "vitest";
import { COMPOSER_CHIP_IDS, DEFAULT_COMPOSER_INLINE_CHIPS, SETTINGS_KEY, defaultEngineFor, loadSettings, sanitizeComposerInlineChips } from "./settings";
import { useTermStore } from "./termStore";

describe("terminal renderer migration", () => {
  beforeEach(() => localStorage.clear());

  it.each(["canvas", "invalid", null])("opens legacy renderer %s with DOM", (termRenderer) => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ termRenderer, gpuRender: true, termFontSize: 18 }));
    const settings = loadSettings();
    expect(settings.termRenderer).toBe("dom");
    expect(settings.termFontSize).toBe(18);
  });

  it.each(["dom", "webgl"])("preserves a supported %s preference", (termRenderer) => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ termRenderer, gpuRender: false }));
    expect(loadSettings().termRenderer).toBe(termRenderer);
  });

  it.each([true, false])("preserves the older GPU preference %s when no renderer was saved", (gpuRender) => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ gpuRender }));
    expect(loadSettings().termRenderer).toBe(gpuRender ? "webgl" : "dom");
  });
});

describe("defaultEngineFor", () => {
  beforeEach(() => localStorage.clear());

  it("opens every chat-capable agent in the conversation", () => {
    expect(defaultEngineFor("claude", {})).toBe("chat");
    expect(defaultEngineFor("codex", {})).toBe("chat");
    expect(defaultEngineFor("opencode", {})).toBe("chat");
    expect(defaultEngineFor("pi", {})).toBe("chat");
    expect(defaultEngineFor("omp", {})).toBe("chat");
  });

  it("prefers the saved per-agent choice over the built-in default", () => {
    expect(defaultEngineFor("omp", { omp: { engine: "tui" } })).toBe("tui");
    expect(defaultEngineFor("pi", { pi: { engine: "tui" } })).toBe("tui");
  });
});

describe("planExecutePrefs sanitization", () => {
  beforeEach(() => localStorage.clear());

  it("keeps only well-formed role choices", () => {
    localStorage.setItem(
      SETTINGS_KEY,
      JSON.stringify({ planExecutePrefs: { plan: { agent: "codex", model: 7, effort: "high" }, exec: "nonsense" } }),
    );
    expect(loadSettings().planExecutePrefs).toEqual({ plan: { agent: "codex", effort: "high" }, exec: {} });
  });

  it("defaults to empty roles when nothing was saved", () => {
    expect(loadSettings().planExecutePrefs).toEqual({ plan: {}, exec: {} });
  });
});

describe("memoryPrefs sanitization", () => {
  beforeEach(() => localStorage.clear());

  it("keeps only a well-formed agent, model and effort choice", () => {
    localStorage.setItem(
      SETTINGS_KEY,
      JSON.stringify({ memoryPrefs: { agent: "codex", model: 7, effort: "high" } }),
    );
    expect(loadSettings().memoryPrefs).toEqual({ agent: "codex", effort: "high" });
  });

  it("defaults to an empty choice when nothing was saved", () => {
    expect(loadSettings().memoryPrefs).toEqual({});
  });
});

describe("reference summary settings", () => {
  beforeEach(() => localStorage.clear());

  it("keeps pre-summary disabled by default", () => {
    expect(loadSettings().referSummary).toEqual({
      enabled: false,
      agent: "claude",
      model: "",
      effort: "",
    });
  });

  it("sanitizes the one global Agent, model, and effort selection", () => {
    localStorage.setItem(
      SETTINGS_KEY,
      JSON.stringify({ referSummary: { enabled: true, agent: "codex", model: 7, effort: "high" } }),
    );
    expect(loadSettings().referSummary).toEqual({
      enabled: true,
      agent: "codex",
      model: "",
      effort: "high",
    });
  });
});

describe("legacy defaultSessionEngine migration", () => {
  beforeEach(() => localStorage.clear());

  it("folds the retired app-wide choice into Claude, Codex and OpenCode", () => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ defaultSessionEngine: "tui" }));
    const s = loadSettings();
    expect(s.agentDefaults.claude?.engine).toBe("tui");
    expect(s.agentDefaults.codex?.engine).toBe("tui");
    expect(s.agentDefaults.opencode?.engine).toBe("tui");
    // Pi and OMP were never covered by the app-wide setting, so the unified fallback still applies.
    expect(s.agentDefaults.pi?.engine).toBeUndefined();
    expect(defaultEngineFor("pi", s.agentDefaults)).toBe("chat");
    expect(defaultEngineFor("omp", s.agentDefaults)).toBe("chat");
  });

  it("keeps an explicit per-agent choice while migrating the rest", () => {
    localStorage.setItem(
      SETTINGS_KEY,
      JSON.stringify({
        defaultSessionEngine: "tui",
        agentDefaults: { claude: { engine: "chat", args: "--model opus" } },
      }),
    );
    const s = loadSettings();
    expect(s.agentDefaults.claude).toEqual({ engine: "chat", args: "--model opus" });
    expect(s.agentDefaults.codex?.engine).toBe("tui");
  });

  it("does not leak migrated values into the built-in defaults", () => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ defaultSessionEngine: "tui" }));
    loadSettings();
    localStorage.clear();
    expect(loadSettings().agentDefaults).toEqual({});
  });
});

describe("composer inline chips", () => {
  beforeEach(() => localStorage.clear());

  it("defaults to the four chips that used to sit beside the message", () => {
    expect(loadSettings().composerInlineChips).toEqual(["model", "effort", "collaboration", "permission"]);
    expect(DEFAULT_COMPOSER_INLINE_CHIPS).toEqual(["model", "effort", "collaboration", "permission"]);
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ termFontSize: 18 }));
    expect(loadSettings().composerInlineChips).toEqual(DEFAULT_COMPOSER_INLINE_CHIPS);
  });

  it("keeps the saved order, drops unknown ids and duplicates, and accepts an empty list", () => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ composerInlineChips: ["account", "bogus", "model", "account", 7] }));
    expect(loadSettings().composerInlineChips).toEqual(["account", "model"]);
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ composerInlineChips: [] }));
    expect(loadSettings().composerInlineChips).toEqual([]);
    expect(sanitizeComposerInlineChips("model")).toEqual(DEFAULT_COMPOSER_INLINE_CHIPS);
    expect(sanitizeComposerInlineChips(null)).toEqual(DEFAULT_COMPOSER_INLINE_CHIPS);
    expect(sanitizeComposerInlineChips([...COMPOSER_CHIP_IDS])).toEqual([...COMPOSER_CHIP_IDS]);
  });

  it("round-trips through the store setter and the cache hydration", () => {
    vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
    useTermStore.getState().setComposerInlineChips(["permission", "model", "mcp"]);
    expect(loadSettings().composerInlineChips).toEqual(["permission", "model", "mcp"]);
    useTermStore.setState({ composerInlineChips: [] });
    useTermStore.getState().hydrateSettingsFromCache();
    expect(useTermStore.getState().composerInlineChips).toEqual(["permission", "model", "mcp"]);
  });
});
