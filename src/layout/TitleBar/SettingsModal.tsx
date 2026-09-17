import { safeError } from "../../ipc/diagnosticSafety";
//! Modal settings page replacing the narrow title-bar SettingsPopover. Category navigation appears
//! on the left and content on the right; backdrop clicks or Escape close it. Settings persist through
//! the store, with applyVisual writing `data-*` attributes to documentElement. The General category
//! also includes the UI locale selector, persisted as vlx-lang (see src/i18n).

import { useEffect, useState } from "react";
import { Backdrop } from "../../components/Backdrop";
import { CompletionModeField } from "./CompletionModeField";
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
  installSpawnSkills,
  listShells,
  type ShellOption,
} from "../../ipc/commands";
import { pushSetting } from "../../ipc/settingsSync";
import { env, platform } from "../../platform";
import type { VelaCommandStatus } from "../../platform/types";
import { COMPOSER_CHIP_IDS, type ComposerChipId } from "../../store/settings";
import { useTermStore, type TermRenderer } from "../../store/termStore";
import { DEFAULT_CONVERSATION_FONT_SIZE, DEFAULT_TERMINAL_FONT_SIZE, DEFAULT_TERMINAL_LINE_HEIGHT } from "../../theme";
import type {
  AccentChoice,
  Density,
  DividerStyle,
  NavLayout,
  PaneStyle,
} from "../../theme";
import { Field, Seg, SectionTitle } from "./settingsParts";
import {
  AgentsPanel,
  ReferenceContextPanel,
  GiteaIntegrationPanel,
  SHOW_GITEA_INTEGRATION,
  ShortcutsPanel,
} from "./settingsPanels";
import {
  CleanImagesField,
  ImagePasteModeField,
  NotificationField,
} from "./settingsBehaviorFields";

/** Accent palette; auto follows the light/dark theme. */
const ACCENT_HEX: Record<string, string> = {
  green: "#3fcf8e",
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

/** Font-size stepper. Clicking the value resets it; null displays autoLabel for density-derived UI size. */
function FontSizeStepper({
  value,
  onChange,
  min,
  max,
  defaultValue,
  autoLabel,
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
  stepSize?: number;
  unit?: string;
  label?: string;
}) {
  const t = useT();
  const display = value == null ? (autoLabel ?? String(defaultValue)) : `${value}${unit}`;
  const step = (d: number) => {
    const autoSize = autoLabel != null
      ? parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--ui-fs"))
      : NaN;
    const base = value ?? (Number.isFinite(autoSize) ? autoSize : defaultValue);
    onChange(Math.max(min, Math.min(max, Math.round((base + d * stepSize) * 10) / 10)));
  };
  // UI size resets to null to follow density; terminal size resets to its explicit default.
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


type Cat = "appearance" | "terminal" | "conversation" | "behavior" | "advanced" | "agents" | "shortcuts" | "general";

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
  const uiFontFamily = useTermStore((s) => s.uiFontFamily);
  const uiFontSize = useTermStore((s) => s.uiFontSize);
  const termFontFamily = useTermStore((s) => s.termFontFamily);
  const termFontSize = useTermStore((s) => s.termFontSize);
  const termLineHeight = useTermStore((s) => s.termLineHeight);
  const chatFontFamily = useTermStore((s) => s.chatFontFamily);
  const chatFontSize = useTermStore((s) => s.chatFontSize);
  const chatLineHeight = useTermStore((s) => s.chatLineHeight);
  const setUiFontFamily = useTermStore((s) => s.setUiFontFamily);
  const setUiFontSize = useTermStore((s) => s.setUiFontSize);
  const setTermFontFamily = useTermStore((s) => s.setTermFontFamily);
  const setTermFontSize = useTermStore((s) => s.setTermFontSize);
  const setTermLineHeight = useTermStore((s) => s.setTermLineHeight);
  const setChatFontFamily = useTermStore((s) => s.setChatFontFamily);
  const setChatFontSize = useTermStore((s) => s.setChatFontSize);
  const setChatLineHeight = useTermStore((s) => s.setChatLineHeight);

  const [cat, setCat] = useState<Cat>("appearance");
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
    { key: "behavior", label: t("settings.catBehavior") },
    { key: "advanced", label: t("settings.catAdvanced") },
    { key: "agents", label: t("settings.catAgents") },
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
        <div style={{ flex: 1, display: "flex", minHeight: 0 }}>
          <div
            style={{
              width: 158,
              flex: "none",
              borderRight: "1px solid var(--border)",
              padding: 8,
              overflowY: "auto",
            }}
          >
            {cats.map((c) => {
              const on = c.key === cat;
              return (
                <button
                  key={c.key}
                  onClick={() => setCat(c.key)}
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
                </button>
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
                <Field label={t("settings.uiFont")}>
                  <FontSelect fonts={fonts} value={uiFontFamily} onChange={setUiFontFamily} label={t("settings.uiFont")} />
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

            {cat === "conversation" && (
              <>
                <SectionTitle>{t("settings.chatTypography")}</SectionTitle>
                <p style={{ color: "var(--text-dim)", fontSize: 12, lineHeight: 1.5, margin: "0 0 12px" }}>
                  {t("settings.chatTypographyHint")}
                </p>
                <Field label={t("settings.chatFont")}>
                  <FontSelect fonts={fonts} value={chatFontFamily} onChange={setChatFontFamily} label={t("settings.chatFont")} />
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
              </>
            )}

            {cat === "behavior" && (
              <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
                <div>
                  <SectionTitle>{t("settings.catBehavior")}</SectionTitle>
                  <Field label={t("settings.tabs")}>
                    <Seg<"single" | "multi">
                      value={singleTabMode ? "single" : "multi"}
                      options={[
                        ["single", t("settings.tabSingle")],
                        ["multi", t("settings.tabMulti")],
                      ]}
                      onChange={(v) => setSingleTabMode(v === "single")}
                    />
                  </Field>
                  <Field label={t("settings.dynamicStatusFilter")}>
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
                    <Field label={t("settings.maxLiveTabs")}>
                      <Seg<string>
                        value={String(maxLiveTabs)}
                        options={[
                          ["8", "8"],
                          ["16", "16"],
                          ["32", "32"],
                          ["64", "64"],
                        ]}
                        onChange={(v) => setMaxLiveTabs(Number(v))}
                      />
                    </Field>
                  )}
                  <Field label={t("settings.spawnConfirm")}>
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

            {cat === "shortcuts" && <ShortcutsPanel />}

            {cat === "general" && (
              <>
                <SectionTitle>{t("settings.catGeneral")}</SectionTitle>
                <Field label={t("settings.language")}>
                  <LangSelect />
                </Field>
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
                          color: cliStatus?.installed ? "var(--text)" : "#fff",
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
                          color: skillOn ? "var(--text-dim)" : "#fff",
                          cursor: "pointer",
                        }}
                        onClick={async () => {
                          try {
                            await installSpawnSkills();
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
