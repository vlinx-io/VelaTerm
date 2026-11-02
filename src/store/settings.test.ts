//! Coverage for the per-agent default view: the built-in fallback, the saved per-agent choice, and the
//! migration from the retired app-wide setting. Every chat-capable agent shares the conversation fallback;
//! saved per-agent choices can still override it.

import { beforeEach, describe, expect, it, vi } from "vitest";
import { COMPOSER_CHIP_IDS, DEFAULT_COMPOSER_INLINE_CHIPS, SETTINGS_KEY, defaultEngineFor, loadSettings, sanitizeComposerInlineChips } from "./settings";
import { useTermStore } from "./termStore";

describe("dark palette preference", () => {
  beforeEach(() => localStorage.clear());

  it.each([undefined, null, "unknown"])("opens old or invalid preference %s with classic dark", (darkStyle) => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ darkStyle, density: "compact" }));
    expect(loadSettings().darkStyle).toBe("classic");
    expect(loadSettings().density).toBe("compact");
  });

  it.each(["soft", "cool", "crisp", "classic"])("normalizes the saved %s preference to classic dark", (darkStyle) => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ darkStyle }));
    expect(loadSettings().darkStyle).toBe("classic");
  });

  it("selects classic dark and retains it across light mode and hydration", () => {
    const { theme, darkStyle } = useTermStore.getState();
    try {
      useTermStore.getState().setDarkTheme("classic");
      expect(useTermStore.getState().theme).toBe("dark");
      expect(JSON.parse(localStorage.getItem(SETTINGS_KEY)!).darkStyle).toBe("classic");
      useTermStore.getState().setTheme("light");
      const saved = JSON.parse(localStorage.getItem(SETTINGS_KEY)!);
      localStorage.setItem(SETTINGS_KEY, JSON.stringify({ ...saved, darkStyle: "crisp" }));
      useTermStore.getState().hydrateSettingsFromCache();
      expect(useTermStore.getState().theme).toBe("light");
      expect(useTermStore.getState().darkStyle).toBe("classic");
      expect(document.documentElement.dataset.darkStyle).toBe("classic");
    } finally {
      useTermStore.setState({ theme, darkStyle });
    }
  });

  it("keeps this window's inspector tab when another client's settings are hydrated", () => {
    const { inspectorTab } = useTermStore.getState();
    vi.stubGlobal("matchMedia", () => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() }));
    try {
      useTermStore.setState({ inspectorTab: "git" });
      localStorage.setItem(SETTINGS_KEY, JSON.stringify({ ...loadSettings(), inspectorTab: "knowledge" }));
      useTermStore.getState().hydrateSettingsFromCache();
      expect(useTermStore.getState().inspectorTab).toBe("git");
    } finally {
      useTermStore.setState({ inspectorTab });
      vi.unstubAllGlobals();
    }
  });
});

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

  it("uses each agent's built-in default view", () => {
    expect(defaultEngineFor("claude", {})).toBe("chat");
    expect(defaultEngineFor("codex", {})).toBe("chat");
    expect(defaultEngineFor("opencode", {})).toBe("tui");
    expect(defaultEngineFor("pi", {})).toBe("tui");
    expect(defaultEngineFor("omp", {})).toBe("tui");
    expect(defaultEngineFor("antigravity", {})).toBe("tui");
  });

  it("prefers the saved per-agent choice over the built-in default", () => {
    expect(defaultEngineFor("omp", { omp: { engine: "chat" } })).toBe("chat");
    expect(defaultEngineFor("pi", { pi: { engine: "chat" } })).toBe("chat");
  });
});

describe("planExecutePrefs sanitization", () => {
  beforeEach(() => localStorage.clear());

  it("keeps only well-formed role choices", () => {
    localStorage.setItem(
      SETTINGS_KEY,
      JSON.stringify({ planExecutePrefs: { plan: { agent: "codex", model: 7, effort: "high" }, exec: "nonsense" } }),
    );
    expect(loadSettings().planExecutePrefs).toEqual({ plan: { agent: "codex", effort: "high" }, exec: {}, review: {} });
  });

  it("defaults to empty roles when nothing was saved", () => {
    expect(loadSettings().planExecutePrefs).toEqual({ plan: {}, exec: {}, review: {} });
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

describe("sessionTitlePrefs sanitization", () => {
  beforeEach(() => localStorage.clear());

  it("keeps well-formed per-agent model and effort choices", () => {
    localStorage.setItem(
      SETTINGS_KEY,
      JSON.stringify({ sessionTitlePrefs: { claude: { model: "opus", effort: 3 }, codex: "gpt-5", grok: { effort: "" } } }),
    );
    expect(loadSettings().sessionTitlePrefs).toEqual({ claude: { model: "opus" }, grok: { effort: "" } });
  });

  it("defaults to no remembered choices", () => {
    expect(loadSettings().sessionTitlePrefs).toEqual({});
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
    // Pi and OMP were never covered by the app-wide setting, so their built-in defaults still apply.
    expect(s.agentDefaults.pi?.engine).toBeUndefined();
    expect(defaultEngineFor("pi", s.agentDefaults)).toBe("tui");
    expect(defaultEngineFor("omp", s.agentDefaults)).toBe("tui");
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

  it("defaults to the four chips that used to sit beside the message, plus Tasks", () => {
    expect(loadSettings().composerInlineChips).toEqual(["model", "effort", "collaboration", "permission", "tasks"]);
    expect(DEFAULT_COMPOSER_INLINE_CHIPS).toEqual(["model", "effort", "collaboration", "permission", "tasks"]);
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ termFontSize: 18 }));
    expect(loadSettings().composerInlineChips).toEqual(DEFAULT_COMPOSER_INLINE_CHIPS);
  });

  it("keeps the saved order, drops unknown ids and duplicates, and accepts an empty list", () => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ composerInlineChips: ["account", "bogus", "model", "account", 7], composerInlineChipsRevision: 1 }));
    expect(loadSettings().composerInlineChips).toEqual(["account", "model"]);
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ composerInlineChips: [], composerInlineChipsRevision: 1 }));
    expect(loadSettings().composerInlineChips).toEqual([]);
    expect(sanitizeComposerInlineChips("model")).toEqual(DEFAULT_COMPOSER_INLINE_CHIPS);
    expect(sanitizeComposerInlineChips(null)).toEqual(DEFAULT_COMPOSER_INLINE_CHIPS);
    expect(sanitizeComposerInlineChips([...COMPOSER_CHIP_IDS])).toEqual([...COMPOSER_CHIP_IDS]);
  });

  it("adds Tasks once to a list saved before it became a default inline chip", () => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ composerInlineChips: ["model", "effort", "collaboration", "permission"] }));
    expect(loadSettings().composerInlineChips).toEqual(["model", "effort", "collaboration", "permission", "tasks"]);
    expect(loadSettings().composerInlineChipsRevision).toBe(1);
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ composerInlineChips: ["tasks", "model"] }));
    expect(loadSettings().composerInlineChips).toEqual(["tasks", "model"]);
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ composerInlineChips: ["model"], composerInlineChipsRevision: 1 }));
    expect(loadSettings().composerInlineChips).toEqual(["model"]);
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

describe("project-scoped tabs preference", () => {
  beforeEach(() => localStorage.clear());

  it("is off by default", () => {
    expect(loadSettings().scopeTabsToProject).toBe(false);
  });

  it("keeps a saved choice and rejects non-boolean values", () => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ scopeTabsToProject: true }));
    expect(loadSettings().scopeTabsToProject).toBe(true);
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ scopeTabsToProject: "yes" }));
    expect(loadSettings().scopeTabsToProject).toBe(false);
  });

  it("persists the store setter", () => {
    const before = useTermStore.getState().scopeTabsToProject;
    try {
      useTermStore.getState().setScopeTabsToProject(true);
      expect(useTermStore.getState().scopeTabsToProject).toBe(true);
      expect(JSON.parse(localStorage.getItem(SETTINGS_KEY)!).scopeTabsToProject).toBe(true);
    } finally {
      useTermStore.setState({ scopeTabsToProject: before });
    }
  });
});
