import { safeError } from "../../ipc/diagnosticSafety";
//! Modal settings page replacing the narrow title-bar SettingsPopover. Category navigation appears
//! on the left and content on the right; backdrop clicks or Escape close it. Settings persist through
//! the store, with applyVisual writing `data-*` attributes to documentElement. The General category
//! also includes the UI locale selector, persisted as vlx-lang (see src/i18n).

import { useEffect, useState } from "react";
import { Backdrop } from "../../components/Backdrop";
import { CompletionModeField } from "./CompletionModeField";
import { ServerRegionField } from "./ServerRegionField";
import { FontSelect, useFontCatalog } from "./FontSelect";
import Select, { type SelectOption } from "../../components/Select";
import {
  getLangChoice,
  LOCALE_NAMES,
  LOCALES,
  setLang,
  useT,
  type LangChoice,
} from "../../i18n";
import {
  spawnSkillsInstalled,
  listShells,
  type ShellOption,
} from "../../ipc/commands";
import { pushSetting } from "../../ipc/settingsSync";
import { installVelaSkills } from "../../ipc/velaSkills";
import { env, platform } from "../../platform";
import type { VelaCommandStatus } from "../../platform/types";
import {
  CHAT_IMAGE_MAX_MB_CHOICES,
  COMPOSER_CHIP_IDS,
  INPUT_LATENCY_THRESHOLDS,
  type ChatImageMaxMb,
  type ComposerChipId,
} from "../../store/settings";
import { useTermStore, type TermRenderer } from "../../store/termStore";
import {
  DEFAULT_CHAT_TRACKING,
  DEFAULT_CONVERSATION_FONT_SIZE,
  DEFAULT_TERMINAL_FONT_SIZE,
  DEFAULT_TERMINAL_LINE_HEIGHT,
  MAX_CHAT_TRACKING,
  MIN_CHAT_TRACKING,
  SETTINGS_TABS,
} from "../../theme";
import type {
  AccentChoice,
  Density,
  DividerStyle,
  NavLayout,
  PaneStyle,
  SettingsTab,
} from "../../theme";
import { Field, Seg, SectionTitle } from "./settingsParts";
import {
  AgentsPanel,
  ReferenceContextPanel,
  GiteaIntegrationPanel,
  SHOW_GITEA_INTEGRATION,
  ShortcutsPanel,
} from "./settingsPanels";
import { PluginsPanel } from "./PluginsPanel";
import { PLUGINS_ENABLED } from "../../plugins/host";
import {
  CleanImagesField,
  ImagePasteModeField,
  NotificationField,
} from "./settingsBehaviorFields";
import { TranslationSettingsSection } from "../../translation/SettingsSection";
import { followTranslationLink, navigateTranslation, translationUrl, useTranslationRoute } from "../../translation/navigation";

/** Accent palette; auto follows the light/dark theme. */
const ACCENT_HEX: Record<string, string> = {
  green: "var(--accent-green-swatch)",
  blue: "#5b9dff",
  amber: "#f5b14c",
  violet: "#b08bff",
};
const ACCENTS: AccentChoice[] = ["auto", "green", "blue", "amber", "violet"];

/** Shared style for the font-size stepper's minus/plus buttons. */


const STEP_BTN: React.CSSProperties = {
  width: 24,
  height: 26,
  display: "grid",
  placeItems: "center",
  background: "var(--bg-active)",
  color: "var(--text)",
  border: "1px solid var(--border)",
  borderRadius: 5,
  fontSize: 14,
  lineHeight: 1,
  cursor: "pointer",
  flex: "none",
};

/** Choices for the background-tab limit dropdown. */
const MAX_LIVE_TABS_CHOICES = [8, 16, 32, 64, 128, 256, 512];

/** Language picker. Writing the choice through to the backend as well keeps other shells in sync. */
function LangSelect() {
  const t = useT();
  const choice = getLangChoice();

  const options: SelectOption<LangChoice>[] = [
    { value: "auto", label: t("settings.langAuto") },
    ...LOCALES.map((loc) => ({ value: loc as LangChoice, label: LOCALE_NAMES[loc] })),
  ];

  return (
    <Select
      value={choice}
      onChange={(v) => {
        setLang(v);
        pushSetting("vlx-lang", v); // Also persist in the backend for cross-shell sharing.
      }}
      options={options}
      size="sm"
      width={160}
      menuWidth={172}
      align="right"
      ariaLabel={t("settings.language")}
    />
  );
}

/** Default-shell picker listing the system default plus detected shells. The row hides itself when
 * detection turns up nothing. New terminals without an explicit choice use this value; tree and
 * edit-form entry points may still override it per launch. */
function ShellSelect() {
  const t = useT();
  const defaultShell = useTermStore((s) => s.defaultShell);
  const setDefaultShell = useTermStore((s) => s.setDefaultShell);
  const [shells, setShells] = useState<ShellOption[]>([]);
  useEffect(() => {
    let alive = true;
    listShells()
      .then((s) => alive && setShells(s))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  if (!shells.length) return null;

  const options = [
    { value: "", label: t("tree.shellSystemDefault") },
    ...shells.map((s) => ({ value: s.path, label: s.label })),
  ];

  return (
    <Field label={t("settings.defaultShell")}>
      <Select
        value={defaultShell}
        onChange={setDefaultShell}
        options={options}
        size="sm"
        width={160}
        menuWidth={172}
        align="right"
        ariaLabel={t("settings.defaultShell")}
      />
    </Field>
  );
}

/** Target compressed byte budget for images sent in conversation view. */
function ChatImagesSection() {
  const t = useT();
  const chatImageMaxMb = useTermStore((s) => s.chatImageMaxMb);
  const setChatImageMaxMb = useTermStore((s) => s.setChatImageMaxMb);
  return (
    <div style={{ marginTop: 20 }}>
      <SectionTitle>{t("settings.chatImages")}</SectionTitle>
      <p style={{ color: "var(--text-dim)", fontSize: 12, lineHeight: 1.5, margin: "0 0 12px" }}>
        {t("settings.chatImageMaxMbHint")}
      </p>
      <Field label={t("settings.chatImageMaxMb")}>
        <Seg<string>
          value={String(chatImageMaxMb)}
          options={CHAT_IMAGE_MAX_MB_CHOICES.map((mb) => [
            String(mb),
            t("settings.chatImageSizeMb", mb),
          ] as [string, string])}
          onChange={(v) => setChatImageMaxMb(Number(v) as ChatImageMaxMb)}
        />
      </Field>
    </div>
  );
}

/** One row per composer chip: the chips that are on come first in their toolbar order, then the rest.
 * Turning a chip on appends it; the arrows swap neighbours within the enabled list. */
function ComposerChipsSection() {
  const t = useT();
  const inline = useTermStore((s) => s.composerInlineChips);
  const setInline = useTermStore((s) => s.setComposerInlineChips);
  const rows: ComposerChipId[] = [...inline, ...COMPOSER_CHIP_IDS.filter((id) => !inline.includes(id))];
  const move = (id: ComposerChipId, delta: -1 | 1) => {
    const from = inline.indexOf(id);
    const to = from + delta;
    if (from < 0 || to < 0 || to >= inline.length) return;
    const next = [...inline];
    next.splice(from, 1);
    next.splice(to, 0, id);
    setInline(next);
  };
  return (
    <div style={{ marginTop: 20 }}>
      <SectionTitle>{t("settings.composerChips")}</SectionTitle>
      <p style={{ color: "var(--text-dim)", fontSize: 12, lineHeight: 1.5, margin: "0 0 12px" }}>
        {t("settings.composerChipsHint")}
      </p>
      {rows.map((id) => {
        const name = t(`settings.composerChip.${id}`);
        const at = inline.indexOf(id);
        const on = at >= 0;
        return (
          <Field key={id} label={name}>
            <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
              <button style={STEP_BTN} disabled={!on || at === 0} aria-label={t("settings.composerChipUp", name)}
                onClick={() => move(id, -1)}>↑</button>
              <button style={STEP_BTN} disabled={!on || at === inline.length - 1} aria-label={t("settings.composerChipDown", name)}
                onClick={() => move(id, 1)}>↓</button>
              <Seg<"on" | "off">
                value={on ? "on" : "off"}
                options={[
                  ["on", t("common.on")],
                  ["off", t("common.off")],
                ]}
                onChange={(v) => setInline(v === "on" ? [...inline, id] : inline.filter((other) => other !== id))}
              />
            </div>
          </Field>
        );
      })}
    </div>
  );
}

function FontSizeStepper({
  value,
  onChange,
  min,
  max,
  defaultValue,
  autoLabel,
  autoVar = "--ui-fs",
  stepSize = 0.5,
  unit = "px",
  label,
}: {
  value: number | null;
  onChange: (v: number | null) => void;
  min: number;
  max: number;
  defaultValue: number;
  autoLabel?: string;
  /** CSS variable on documentElement holding the value currently in effect while the setting is automatic. */
  autoVar?: string;
  stepSize?: number;
  unit?: string;
  label?: string;
}) {
  const t = useT();
  const display = value == null ? (autoLabel ?? String(defaultValue)) : `${value}${unit}`;
  const step = (d: number) => {
    // Stepping out of automatic starts from what is on screen rather than jumping to the fallback.
    const autoSize = autoLabel != null
      ? parseFloat(getComputedStyle(document.documentElement).getPropertyValue(autoVar))
      : NaN;
    const base = value ?? (Number.isFinite(autoSize) ? autoSize : defaultValue);
    onChange(Math.max(min, Math.min(max, Math.round((base + d * stepSize) * 10) / 10)));
  };
  // Automatic values reset to null; terminal size resets to its explicit default.
  const reset = () => onChange(autoLabel != null ? null : defaultValue);
  return (
    <div role={label ? "group" : undefined} aria-label={label} style={{ display: "flex", alignItems: "center", gap: 4 }}>
      <button style={STEP_BTN} onClick={() => step(-1)} title={t("settings.fontSmaller")}>
        −
      </button>
      <button
        onClick={reset}
        title={t("settings.fontReset")}
        style={{
          minWidth: 52,
          height: 26,
          padding: "0 6px",
          background: "var(--bg-active)",
          color: "var(--text)",
          border: "1px solid var(--border)",
          borderRadius: 5,
          fontSize: 11.5,
          cursor: "pointer",
          fontVariantNumeric: "tabular-nums",
        }}
      >
        {display}
      </button>
      <button style={STEP_BTN} onClick={() => step(1)} title={t("settings.fontLarger")}>
        +
      </button>
    </div>
  );
}

/** Code shown in the previews. It is program text, identical in every language. */
const PREVIEW_CODE = "const total = items.reduce((sum, x) => sum + x, 0); // 0O 1lI";

/** Live sample of the interface or conversation typography. It reads the same CSS variables as the real
 * views, so every change above shows here at once without a separate code path. */
function TypographyPreview({ variant }: { variant: "ui" | "chat" }) {
  const t = useT();
  const box: React.CSSProperties = {
    margin: "12px 0 0",
    padding: "10px 12px",
    border: "1px solid var(--border)",
    borderRadius: 8,
    background: variant === "chat" ? "var(--bg-term)" : "var(--bg-0)",
    color: "var(--text)",
    overflow: "hidden",
  };
  const caption = (
    <div style={{ fontFamily: "var(--font-ui)", fontSize: 10.5, color: "var(--text-dim)", marginBottom: 6 }}>
      {t("settings.fontPreview")}
    </div>
  );
  if (variant === "ui") {
    return (
      <div role="group" aria-label={t("settings.fontPreview")} style={box}>
        {caption}
        <div style={{ fontFamily: "var(--font-ui)", fontSize: "var(--ui-fs)", lineHeight: 1.5 }}>
          {t("settings.fontPreviewText")}
        </div>
        <div style={{ fontFamily: "var(--font-mono)", fontSize: "calc(var(--ui-fs) - 1px)", lineHeight: 1.5, color: "var(--text-dim)", whiteSpace: "nowrap", textOverflow: "ellipsis", overflow: "hidden" }}>
          {PREVIEW_CODE}
        </div>
      </div>
    );
  }
  return (
    <div role="group" aria-label={t("settings.fontPreview")} style={box}>
      {caption}
      <div style={{ fontFamily: "var(--chat-font)", fontSize: "var(--chat-fs)", lineHeight: "var(--chat-line-height)", letterSpacing: "var(--chat-tracking)" }}>
        {t("settings.fontPreviewChat")}
      </div>
      <pre style={{ margin: "8px 0 0", padding: "6px 8px", borderRadius: 6, background: "var(--bg-active)", fontFamily: "var(--chat-code-font)", fontSize: "calc(var(--ui-fs) - 1px)", lineHeight: "var(--chat-line-height)", whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>
        {PREVIEW_CODE}
      </pre>
    </div>
  );
}

/** Restores one group of font settings at once. */
function FontResetRow({ onReset }: { onReset: () => void }) {
  const t = useT();
  return (
    <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 10 }}>
      <button type="button" className="vlx-btn" onClick={onReset}>
        {t("settings.fontResetGroup")}
      </button>
    </div>
  );
}

/** Accent-color palette. */
function AccentPicker({
  accent,
  setAccent,
}: {
  accent: AccentChoice;
  setAccent: (a: AccentChoice) => void;
}) {
  const t = useT();
  return (
    <div style={{ display: "flex", gap: 6 }}>
      {ACCENTS.map((a) => {
        const on = accent === a;
        return (
          <button
            key={a}
            title={a === "auto" ? t("settings.accentAuto") : a}
            onClick={() => setAccent(a)}
            style={{
              width: 22,
              height: 22,
              borderRadius: 5,
              display: "grid",
              placeItems: "center",
              border: on ? "2px solid var(--accent)" : "1px solid var(--border)",
              background: a === "auto" ? "var(--bg-active)" : ACCENT_HEX[a],
              color: "var(--bg-0)",
              fontSize: 9,
              fontWeight: 700,
              cursor: "pointer",
            }}
          >
            {a === "auto" ? <span style={{ color: "var(--text-dim)" }}>A</span> : null}
          </button>
        );
      })}
    </div>
  );
}


type Cat = SettingsTab | "translation";

const CATS = new Set<Cat>([
  "general",
  "appearance",
  "terminal",
  "conversation",
  "translation",
  "behavior",
  "advanced",
  "agents",
  ...(PLUGINS_ENABLED ? (["plugins"] as const) : []),
  "shortcuts",
]);

/** A category from the URL, or null for a missing or retired one so no blank page can be opened. */
function categoryOf(value: string | null): Cat | null {
  return value !== null && CATS.has(value as Cat) ? (value as Cat) : null;
}

export function SettingsModal({ onClose }: { onClose: () => void }) {
  const fonts = useFontCatalog();
  const t = useT();
  const accent = useTermStore((s) => s.accent);
  const density = useTermStore((s) => s.density);
  const paneStyle = useTermStore((s) => s.paneStyle);
  const dividerStyle = useTermStore((s) => s.dividerStyle);
  const navLayout = useTermStore((s) => s.navLayout);
  const setAccent = useTermStore((s) => s.setAccent);
  const setDensity = useTermStore((s) => s.setDensity);
  const setPaneStyle = useTermStore((s) => s.setPaneStyle);
  const setDividerStyle = useTermStore((s) => s.setDividerStyle);
  const setNavLayout = useTermStore((s) => s.setNavLayout);
  const singleTabMode = useTermStore((s) => s.singleTabMode);
  const setSingleTabMode = useTermStore((s) => s.setSingleTabMode);
  const spawnConfirm = useTermStore((s) => s.spawnConfirm);
  const setSpawnConfirm = useTermStore((s) => s.setSpawnConfirm);
  const termRenderer = useTermStore((s) => s.termRenderer);
  const setTermRenderer = useTermStore((s) => s.setTermRenderer);
  const redrawOnReveal = useTermStore((s) => s.redrawOnReveal);
  const setRedrawOnReveal = useTermStore((s) => s.setRedrawOnReveal);
  const outputScheduler = useTermStore((s) => s.outputScheduler);
  const setOutputScheduler = useTermStore((s) => s.setOutputScheduler);
  const inputLatencyLog = useTermStore((s) => s.inputLatencyLog);
  const setInputLatencyLog = useTermStore((s) => s.setInputLatencyLog);
  const inputLatencyThresholdMs = useTermStore((s) => s.inputLatencyThresholdMs);
  const setInputLatencyThresholdMs = useTermStore((s) => s.setInputLatencyThresholdMs);
  const dynamicStatusFilter = useTermStore((s) => s.dynamicStatusFilter);
  const setDynamicStatusFilter = useTermStore((s) => s.setDynamicStatusFilter);
  const recordSessions = useTermStore((s) => s.recordSessions);
  const setRecordSessions = useTermStore((s) => s.setRecordSessions);
  const maxLiveTabs = useTermStore((s) => s.maxLiveTabs);
  const setMaxLiveTabs = useTermStore((s) => s.setMaxLiveTabs);
  const usageAutoRefresh = useTermStore((s) => s.usageAutoRefresh);
  const setUsageAutoRefresh = useTermStore((s) => s.setUsageAutoRefresh);
  const usageRefreshSec = useTermStore((s) => s.usageRefreshSec);
  const setUsageRefreshSec = useTermStore((s) => s.setUsageRefreshSec);
  const autoContinueAtUsageLimit = useTermStore((s) => s.autoContinueAtUsageLimit);
  const setAutoContinueAtUsageLimit = useTermStore((s) => s.setAutoContinueAtUsageLimit);
  const soundEnabled = useTermStore((s) => s.soundEnabled);
  const toggleSound = useTermStore((s) => s.toggleSound);
  const uiFontSize = useTermStore((s) => s.uiFontSize);
  const uiFontFamily = useTermStore((s) => s.uiFontFamily);
  const uiMonoFontFamily = useTermStore((s) => s.uiMonoFontFamily);
  const chatCodeFontFamily = useTermStore((s) => s.chatCodeFontFamily);
  const chatLetterSpacing = useTermStore((s) => s.chatLetterSpacing);
  const setUiFontFamily = useTermStore((s) => s.setUiFontFamily);
  const setUiMonoFontFamily = useTermStore((s) => s.setUiMonoFontFamily);
  const resetUiTypography = useTermStore((s) => s.resetUiTypography);
  const setChatCodeFontFamily = useTermStore((s) => s.setChatCodeFontFamily);
  const setChatLetterSpacing = useTermStore((s) => s.setChatLetterSpacing);
  const resetChatTypography = useTermStore((s) => s.resetChatTypography);
  const termFontFamily = useTermStore((s) => s.termFontFamily);
  const termFontSize = useTermStore((s) => s.termFontSize);
  const termLineHeight = useTermStore((s) => s.termLineHeight);
  const chatFontFamily = useTermStore((s) => s.chatFontFamily);
  const chatFontSize = useTermStore((s) => s.chatFontSize);
  const chatLineHeight = useTermStore((s) => s.chatLineHeight);
  const setUiFontSize = useTermStore((s) => s.setUiFontSize);
  const setTermFontFamily = useTermStore((s) => s.setTermFontFamily);
  const setTermFontSize = useTermStore((s) => s.setTermFontSize);
  const setTermLineHeight = useTermStore((s) => s.setTermLineHeight);
  const setChatFontFamily = useTermStore((s) => s.setChatFontFamily);
  const setChatFontSize = useTermStore((s) => s.setChatFontSize);
  const setChatLineHeight = useTermStore((s) => s.setChatLineHeight);

  const route = useTranslationRoute();
  const legacyTranslation = route.get("settings") === "conversation" && route.get("settingsSection") === "translation";
  const requestedCategory = legacyTranslation ? "translation" : categoryOf(route.get("settings"));
  const showTranslation = route.get("settingsTranslation") === "1" || requestedCategory === "translation";
  // The section lives in local state so switching panels does not go through settings persistence; the
  // store only seeds it on open and records each change, so Settings reopens where it was left.
  // Translation is not recorded: it is listed only when the route asks for it, so reopening on it would
  // show a panel with no entry in the list.
  const savedTab = useTermStore((s) => s.settingsTab);
  const setSettingsTab = useTermStore((s) => s.setSettingsTab);
  const initialCat: Cat = requestedCategory ?? (CATS.has(savedTab) ? savedTab : "appearance");
  const [cat, setCatLocal] = useState<Cat>(initialCat);
  const setCat = (v: Cat) => {
    setCatLocal(v);
    if ((SETTINGS_TABS as readonly string[]).includes(v)) setSettingsTab(v as SettingsTab);
  };
  useEffect(() => { if (requestedCategory) setCat(requestedCategory as Cat); }, [requestedCategory]);
  useEffect(() => { if (legacyTranslation) navigateTranslation(translationUrl({ settings: "translation", settingsSection: null }), true); }, [legacyTranslation]);
  const [skillOn, setSkillOn] = useState<boolean | null>(null);
  const [cliStatus, setCliStatus] = useState<VelaCommandStatus | null>(null);
  const [cliBusy, setCliBusy] = useState(false);
  const [cliError, setCliError] = useState<string | null>(null);
  useEffect(() => {
    spawnSkillsInstalled().then(setSkillOn);
    if (!env.isDev && env.isMac && (env.isTauri || env.isElectron)) {
      platform.velaCommand.status().then(setCliStatus).catch((e) => setCliError(String(e)));
    }
  }, []);

  const cats: { key: Cat; label: string }[] = [
    { key: "general", label: t("settings.catGeneral") },
    { key: "appearance", label: t("settings.appearance") },
    { key: "terminal", label: t("settings.catTerminal") },
    { key: "conversation", label: t("settings.chatTypography") },
    { key: "translation", label: t("translation.title") },
    { key: "behavior", label: t("settings.catBehavior") },
    { key: "advanced", label: t("settings.catAdvanced") },
    { key: "agents", label: t("settings.catAgents") },
    ...(PLUGINS_ENABLED ? [{ key: "plugins" as const, label: t("settings.catPlugins") }] : []),
    { key: "shortcuts", label: t("settings.catShortcuts") },
  ];

  return (
    <Backdrop onClose={onClose}>
      <div
        role="dialog"
        aria-modal="true"
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Escape") onClose();
        }}
        style={{
          width: 720,
          maxWidth: "92vw",
          height: 560,
          maxHeight: "86vh",
          display: "flex",
          flexDirection: "column",
          background: "var(--bg-2)",
          border: "1px solid var(--border-strong)",
          borderRadius: "var(--r-md)",
          boxShadow: "var(--shadow)",
          overflow: "hidden",
        }}
      >
        {/* Header */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "13px 16px",
            borderBottom: "1px solid var(--border)",
            flex: "none",
          }}
        >
          <span style={{ fontSize: 14, fontWeight: 600, color: "var(--text)" }}>
            {t("settings.title")}
          </span>
          <button
            onClick={onClose}
            title={t("common.close")}
            style={{
              width: 26,
              height: 26,
              display: "grid",
              placeItems: "center",
              background: "transparent",
              color: "var(--text-dim)",
              border: "none",
              borderRadius: 6,
              fontSize: 15,
              lineHeight: 1,
              cursor: "pointer",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = "var(--bg-active)";
              e.currentTarget.style.color = "var(--text)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = "transparent";
              e.currentTarget.style.color = "var(--text-dim)";
            }}
          >
            ✕
          </button>
        </div>

        {/* Main area: category navigation and content. */}
        <div className="settings-modal-body" style={{ flex: 1, display: "flex", minHeight: 0 }}>
          <div className="settings-modal-nav"
            style={{
              width: 158,
              flex: "none",
              borderRight: "1px solid var(--border)",
              padding: 8,
              overflowY: "auto",
            }}
          >
            {cats.filter(c => c.key !== "translation" || showTranslation).map((c) => {
              const on = c.key === cat;
              return (
                <a
                  key={c.key}
                  href={translationUrl({ settings: c.key, settingsSection: null, settingsTranslation: showTranslation ? "1" : null, translationService: null })}
                  onClick={(event) => { followTranslationLink(event); if (event.defaultPrevented) setCat(c.key); }}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    width: "100%",
                    padding: "8px 10px",
                    marginBottom: 2,
                    borderRadius: 6,
                    border: "none",
                    textAlign: "left",
                    fontSize: 12.5,
                    cursor: "pointer",
                    background: on ? "var(--accent-soft)" : "transparent",
                    color: on ? "var(--accent)" : "var(--text)",
                    fontWeight: on ? 600 : 500,
                    textDecoration: "none",
                  }}
                  onMouseEnter={(e) => {
                    if (!on)
                      e.currentTarget.style.background = "var(--bg-3, rgba(128,128,128,0.10))";
                  }}
                  onMouseLeave={(e) => {
                    if (!on) e.currentTarget.style.background = "transparent";
                  }}
                >
                  {c.label}
                </a>
              );
            })}
          </div>

          <div style={{ flex: 1, overflowY: "auto", padding: "14px 22px 22px" }}>
            {cat === "appearance" && (
              <>
                <SectionTitle>{t("settings.appearance")}</SectionTitle>
                <Field label={t("settings.accent")}>
                  <AccentPicker accent={accent} setAccent={setAccent} />
                </Field>
                <Field label={t("settings.density")}>
                  <Seg<Density>
                    value={density}
                    options={[
                      ["compact", t("settings.densityCompact")],
                      ["regular", t("settings.densityRegular")],
                      ["comfy", t("settings.densityComfy")],
                    ]}
                    onChange={setDensity}
                  />
                </Field>
                <Field label={t("settings.pane")}>
                  <Seg<PaneStyle>
                    value={paneStyle}
                    options={[
                      ["flush", t("settings.paneFlush")],
                      ["card", t("settings.paneCard")],
                    ]}
                    onChange={setPaneStyle}
                  />
                </Field>
                <Field label={t("settings.divider")}>
                  <Seg<DividerStyle>
                    value={dividerStyle}
                    options={[
                      ["subtle", t("settings.dividerSubtle")],
                      ["visible", t("settings.dividerVisible")],
                    ]}
                    onChange={setDividerStyle}
                  />
                </Field>
                <Field label={t("settings.nav")}>
                  <Seg<NavLayout>
                    value={navLayout}
                    options={[
                      ["tree", t("settings.navTree")],
                      ["compact", t("settings.navCompact")],
                    ]}
                    onChange={setNavLayout}
                  />
                </Field>
                <div style={{ marginTop: 20 }}>
                  <SectionTitle>{t("settings.uiTypography")}</SectionTitle>
                  <p style={{ color: "var(--text-dim)", fontSize: 12, lineHeight: 1.5, margin: "0 0 12px" }}>
                    {t("settings.uiTypographyHint")}
                  </p>
                  <Field label={t("settings.uiFont")}>
                    <FontSelect fonts={fonts} value={uiFontFamily} onChange={setUiFontFamily} label={t("settings.uiFont")} />
                  </Field>
                  <Field label={t("settings.uiMonoFont")}>
                    <FontSelect fonts={fonts} value={uiMonoFontFamily} onChange={setUiMonoFontFamily} label={t("settings.uiMonoFont")} />
                  </Field>
                  <Field label={t("settings.uiFontSize")}>
                    <FontSizeStepper
                      value={uiFontSize}
                      label={t("settings.uiFontSize")}
                      onChange={setUiFontSize}
                      min={10}
                      max={20}
                      defaultValue={13}
                      autoLabel={t("settings.fontAuto")}
                    />
                  </Field>
                  <TypographyPreview variant="ui" />
                  <FontResetRow onReset={resetUiTypography} />
                </div>
              </>
            )}

            {cat === "terminal" && (
              <>
                <SectionTitle>{t("settings.catTerminal")}</SectionTitle>
                <ImagePasteModeField />
                <CompletionModeField />
                <ShellSelect />
                <Field label={t("settings.termFont")}>
                  <FontSelect fonts={fonts} value={termFontFamily} onChange={setTermFontFamily} label={t("settings.termFont")} />
                </Field>
                <Field label={t("settings.termFontSize")}>
                  <FontSizeStepper
                    value={termFontSize}
                    label={t("settings.termFontSize")}
                    onChange={(v) => setTermFontSize(v ?? DEFAULT_TERMINAL_FONT_SIZE)}
                    min={10}
                    max={24}
                    defaultValue={DEFAULT_TERMINAL_FONT_SIZE}
                  />
                </Field>
                <Field label={t("settings.termLineHeight")}>
                  <FontSizeStepper value={termLineHeight} label={t("settings.termLineHeight")}
                    onChange={(v) => setTermLineHeight(v ?? DEFAULT_TERMINAL_LINE_HEIGHT)}
                    min={1} max={2} defaultValue={DEFAULT_TERMINAL_LINE_HEIGHT} stepSize={0.1} unit="×" />
                </Field>
              </>
            )}

            {cat === "translation" && <TranslationSettingsSection />}

            {cat === "conversation" && (
              <>
                <SectionTitle>{t("settings.chatTypography")}</SectionTitle>
                <p style={{ color: "var(--text-dim)", fontSize: 12, lineHeight: 1.5, margin: "0 0 12px" }}>
                  {t("settings.chatTypographyHint")}
                </p>
                <Field label={t("settings.chatFont")}>
                  <FontSelect fonts={fonts} value={chatFontFamily} onChange={setChatFontFamily} label={t("settings.chatFont")} />
                </Field>
                <Field label={t("settings.chatCodeFont")}>
                  <FontSelect fonts={fonts} value={chatCodeFontFamily} onChange={setChatCodeFontFamily} label={t("settings.chatCodeFont")} />
                </Field>
                <Field label={t("settings.chatFontSize")}>
                  <FontSizeStepper value={chatFontSize} label={t("settings.chatFontSize")}
                    onChange={(v) => setChatFontSize(v ?? DEFAULT_CONVERSATION_FONT_SIZE)}
                    min={10} max={24} defaultValue={DEFAULT_CONVERSATION_FONT_SIZE} />
                </Field>
                <Field label={t("settings.chatLineHeight")}>
                  <FontSizeStepper value={chatLineHeight} label={t("settings.chatLineHeight")}
                    onChange={(v) => setChatLineHeight(v ?? DEFAULT_TERMINAL_LINE_HEIGHT)}
                    min={1} max={2} defaultValue={DEFAULT_TERMINAL_LINE_HEIGHT} stepSize={0.1} unit="×" />
                </Field>
                <Field label={t("settings.chatLetterSpacing")}>
                  <FontSizeStepper value={chatLetterSpacing} label={t("settings.chatLetterSpacing")}
                    onChange={setChatLetterSpacing} autoLabel={t("settings.fontAuto")} autoVar="--chat-tracking"
                    min={MIN_CHAT_TRACKING} max={MAX_CHAT_TRACKING} defaultValue={DEFAULT_CHAT_TRACKING} stepSize={0.1} />
                </Field>
                <TypographyPreview variant="chat" />
                <FontResetRow onReset={resetChatTypography} />
                <ChatImagesSection />
                <ComposerChipsSection />
              </>
            )}

            {cat === "advanced" && (
              <>
                <SectionTitle>{t("settings.catAdvanced")}</SectionTitle>
                <Field label={t("settings.renderer")}>
                  <Seg<TermRenderer>
                    value={termRenderer}
                    options={[
                      ["dom", "DOM"],
                      ["webgl", "WebGL"],
                    ]}
                    onChange={setTermRenderer}
                  />
                </Field>
                <Field label={t("settings.redrawOnReveal")}>
                  <Seg<"on" | "off">
                    value={redrawOnReveal ? "on" : "off"}
                    options={[
                      ["on", t("common.on")],
                      ["off", t("common.off")],
                    ]}
                    onChange={(v) => setRedrawOnReveal(v === "on")}
                  />
                </Field>
                <Field label={t("settings.outputScheduler")}>
                  <Seg<"on" | "off">
                    value={outputScheduler ? "on" : "off"}
                    options={[
                      ["on", t("common.on")],
                      ["off", t("common.off")],
                    ]}
                    onChange={(v) => setOutputScheduler(v === "on")}
                  />
                </Field>
                <Field label={t("settings.recordSessions")}>
                  <Seg<"on" | "off">
                    value={recordSessions ? "on" : "off"}
                    options={[
                      ["on", t("common.on")],
                      ["off", t("common.off")],
                    ]}
                    onChange={(v) => setRecordSessions(v === "on")}
                  />
                </Field>
                <div
                  style={{
                    marginTop: 8,
                    fontSize: 11,
                    lineHeight: 1.5,
                    color: "var(--text-dim)",
                  }}
                >
                  {t("settings.recordSessionsHint")}
                </div>
                <Field label={t("settings.inputLatencyLog")}>
                  <Seg<"on" | "off">
                    value={inputLatencyLog ? "on" : "off"}
                    options={[
                      ["on", t("common.on")],
                      ["off", t("common.off")],
                    ]}
                    onChange={(v) => setInputLatencyLog(v === "on")}
                  />
                </Field>
                {inputLatencyLog && (
                  <Field label={t("settings.inputLatencyThreshold")}>
                    <Seg<string>
                      value={String(inputLatencyThresholdMs)}
                      options={INPUT_LATENCY_THRESHOLDS.map((ms) => [String(ms), `${ms} ms`] as [string, string])}
                      onChange={(v) => setInputLatencyThresholdMs(Number(v))}
                    />
                  </Field>
                )}
                <div
                  style={{
                    marginTop: 8,
                    fontSize: 11,
                    lineHeight: 1.5,
                    color: "var(--text-dim)",
                  }}
                >
                  {t("settings.inputLatencyLogHint")}
                </div>
              </>
            )}

            {cat === "behavior" && (
              <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
                <div>
                  <SectionTitle>{t("settings.catBehavior")}</SectionTitle>
                  <Field label={t("settings.tabs")} hint={t("settings.tabsHint")}>
                    <Seg<"single" | "multi">
                      value={singleTabMode ? "single" : "multi"}
                      options={[
                        ["single", t("settings.tabSingle")],
                        ["multi", t("settings.tabMulti")],
                      ]}
                      onChange={(v) => setSingleTabMode(v === "single")}
                    />
                  </Field>
                  <Field label={t("settings.dynamicStatusFilter")} hint={t("settings.dynamicStatusFilterHint")}>
                    <Seg<"on" | "off">
                      value={dynamicStatusFilter ? "on" : "off"}
                      options={[
                        ["on", t("common.on")],
                        ["off", t("common.off")],
                      ]}
                      onChange={(v) => setDynamicStatusFilter(v === "on")}
                    />
                  </Field>
                  {singleTabMode && (
                    <Field label={t("settings.maxLiveTabs")} hint={t("settings.maxLiveTabsHint")}>
                      <Seg<string>
                        value={String(maxLiveTabs)}
                        options={MAX_LIVE_TABS_CHOICES.map((n) => [String(n), String(n)] as [string, string])}
                        onChange={(v) => setMaxLiveTabs(Number(v))}
                      />
                    </Field>
                  )}
                  <Field label={t("settings.spawnConfirm")} hint={t("settings.spawnConfirmHint")}>
                    <Seg<"on" | "off">
                      value={spawnConfirm ? "on" : "off"}
                      options={[
                        ["on", t("common.on")],
                        ["off", t("common.off")],
                      ]}
                      onChange={(v) => setSpawnConfirm(v === "on")}
                    />
                  </Field>
                  <Field label={t("settings.usageAuto")}>
                    <Seg<"on" | "off">
                      value={usageAutoRefresh ? "on" : "off"}
                      options={[
                        ["on", t("common.on")],
                        ["off", t("common.off")],
                      ]}
                      onChange={(v) => setUsageAutoRefresh(v === "on")}
                    />
                  </Field>
                  {usageAutoRefresh && (
                    <Field label={t("settings.usageRefresh")}>
                      <Seg<string>
                        value={String(usageRefreshSec)}
                        options={[
                          ["30", "30s"],
                          ["60", "1m"],
                          ["120", "2m"],
                          ["300", "5m"],
                        ]}
                        onChange={(v) => setUsageRefreshSec(Number(v))}
                      />
                    </Field>
                  )}
                  <Field label={t("settings.autoContinue")}>
                    <Seg<"on" | "off">
                      value={autoContinueAtUsageLimit ? "on" : "off"}
                      options={[
                        ["on", t("common.on")],
                        ["off", t("common.off")],
                      ]}
                      onChange={(v) => setAutoContinueAtUsageLimit(v === "on")}
                    />
                  </Field>
                  <div
                    style={{
                      marginTop: 4,
                      marginBottom: 8,
                      fontSize: 11,
                      lineHeight: 1.5,
                      color: "var(--text-dim)",
                    }}
                  >
                    {t("settings.autoContinueHint")}
                  </div>
                  <CleanImagesField />
                </div>
                <ReferenceContextPanel />
              </div>
            )}

            {cat === "agents" && <AgentsPanel />}

            {PLUGINS_ENABLED && cat === "plugins" && <PluginsPanel />}

            {cat === "shortcuts" && <ShortcutsPanel />}

            {cat === "general" && (
              <>
                <SectionTitle>{t("settings.catGeneral")}</SectionTitle>
                <Field label={t("settings.language")}>
                  <LangSelect />
                </Field>
                <ServerRegionField />
                <Field label={t("settings.sound")}>
                  <Seg<"on" | "off">
                    value={soundEnabled ? "on" : "off"}
                    options={[
                      ["on", t("common.on")],
                      ["off", t("common.off")],
                    ]}
                    onChange={(v) => {
                      if ((v === "on") !== soundEnabled) toggleSound();
                    }}
                  />
                </Field>
                <NotificationField />
                {!env.isDev && env.isMac && (env.isTauri || env.isElectron) && (
                  <Field label={t("settings.cliLabel")}>
                    <div
                      style={{
                        display: "flex",
                        flexDirection: "column",
                        alignItems: "flex-end",
                        gap: 6,
                      }}
                    >
                      <button
                        disabled={cliBusy || cliStatus === null}
                        style={{
                          padding: "4px 14px",
                          fontSize: 11.5,
                          borderRadius: 5,
                          border: "1px solid var(--accent)",
                          background: cliStatus?.installed ? "var(--bg-active)" : "var(--accent)",
                          color: cliStatus?.installed ? "var(--text)" : "var(--text-on-accent)",
                          cursor: cliBusy ? "wait" : "pointer",
                          opacity: cliStatus === null ? 0.55 : 1,
                        }}
                        onClick={async () => {
                          setCliBusy(true);
                          setCliError(null);
                          try {
                            const next = cliStatus?.installed
                              ? await platform.velaCommand.uninstall()
                              : await platform.velaCommand.install();
                            setCliStatus(next);
                          } catch (e) {
                            setCliError(String(e));
                            try {
                              setCliStatus(await platform.velaCommand.status());
                            } catch {
                              // Preserve the original state and first error.
                            }
                          } finally {
                            setCliBusy(false);
                          }
                        }}
                      >
                        {cliStatus?.installed
                          ? t("settings.cliUninstall")
                          : t("settings.cliInstall")}
                      </button>
                      <span
                        style={{
                          maxWidth: 430,
                          color: cliError || cliStatus?.conflict ? "#e5484d" : "var(--text-dim)",
                          fontSize: 10.5,
                          lineHeight: 1.45,
                          textAlign: "right",
                          overflowWrap: "anywhere",
                        }}
                      >
                        {cliError
                          ? cliError
                          : cliStatus?.installed && cliStatus.path
                            ? t("settings.cliInstalledAt", cliStatus.path)
                            : cliStatus?.conflict
                              ? t("settings.cliConflict", cliStatus.conflict)
                              : t("settings.cliHint")}
                      </span>
                    </div>
                  </Field>
                )}
                {skillOn !== null && (
                  <Field label={t("settings.skillLabel")}>
                    <div
                      style={{
                        display: "flex",
                        flexDirection: "column",
                        alignItems: "flex-end",
                        gap: 6,
                      }}
                    >
                      <button
                        style={{
                          padding: "4px 14px",
                          fontSize: 11.5,
                          borderRadius: 5,
                          border: skillOn
                            ? "1px solid var(--border)"
                            : "1px solid var(--accent)",
                          background: skillOn ? "var(--bg-active)" : "var(--accent)",
                          color: skillOn ? "var(--text-dim)" : "var(--text-on-accent)",
                          cursor: "pointer",
                        }}
                        onClick={async () => {
                          try {
                            await installVelaSkills();
                            setSkillOn(true);
                          } catch (e) {
                            console.error("vlx skill failed:", safeError(e));
                          }
                        }}
                      >
                        {skillOn ? t("settings.skillInstalled") : t("settings.skillInstall")}
                      </button>
                      <span
                        style={{
                          maxWidth: 430,
                          color: "var(--text-dim)",
                          fontSize: 10.5,
                          lineHeight: 1.45,
                          textAlign: "right",
                        }}
                      >
                        {t("settings.skillInvokeHint")}
                      </span>
                    </div>
                  </Field>
                )}
                {SHOW_GITEA_INTEGRATION && (
                  <>
                    <div style={{ height: 18 }} />
                    <GiteaIntegrationPanel />
                  </>
                )}
              </>
            )}
          </div>
        </div>
      </div>
    </Backdrop>
  );
}
