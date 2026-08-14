//! Modal settings page replacing the narrow title-bar SettingsPopover. Category navigation appears
//! on the left and content on the right; backdrop clicks or Escape close it. Settings persist through
//! the store, with applyVisual writing `data-*` attributes to documentElement. The General category
//! also includes the UI locale selector, persisted as vlx-lang (see src/i18n).

import { useEffect, useState } from "react";
import { Backdrop } from "../../components/Backdrop";
import Icons from "../../components/Icons";
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
import { useTermStore, type TermRenderer } from "../../store/termStore";
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
  GiteaIntegrationPanel,
  OrchestrationPanel,
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

/** Preset monospace fonts; fontStack preserves a monospace fallback when a font is unavailable. */
const MONO_FONTS = [
  "JetBrains Mono",
  "SF Mono",
  "Menlo",
  "Monaco",
  "Consolas",
  "Cascadia Code",
  "Fira Code",
  "Source Code Pro",
  "Hack",
  "Ubuntu Mono",
];

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


/** Custom language dropdown. WKWebView forces native selects into a light beveled style that clashes
 * with the theme, so this uses a button and popup list. */
function LangSelect() {
  const t = useT();
  const choice = getLangChoice();
  const [open, setOpen] = useState(false);

  const options: { value: LangChoice; label: string }[] = [
    { value: "auto", label: t("settings.langAuto") },
    ...LOCALES.map((loc) => ({ value: loc as LangChoice, label: LOCALE_NAMES[loc] })),
  ];
  const current = options.find((o) => o.value === choice)?.label ?? choice;

  return (
    <div style={{ position: "relative" }}>
      <button
        onClick={() => setOpen((o) => !o)}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          width: 160,
          height: 26,
          padding: "0 8px",
          background: "var(--bg-active)",
          color: "var(--text)",
          border: `1px solid ${open ? "var(--accent)" : "var(--border)"}`,
          borderRadius: 6,
          fontSize: 11.5,
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
        <Icons.chevD size={12} style={{ color: "var(--text-dim)", flex: "none" }} />
      </button>

      {open && (
        <>
          {/* Transparent backdrop closes the dropdown; the outer card stops mousedown before it closes the modal. */}
          <div
            style={{ position: "fixed", inset: 0, zIndex: 1140 }}
            onMouseDown={() => setOpen(false)}
          />
          <div
            style={{
              position: "absolute",
              top: "calc(100% + 4px)",
              right: 0,
              zIndex: 1150,
              width: 172,
              maxHeight: 260,
              overflowY: "auto",
              padding: 4,
              background: "var(--bg-2)",
              border: "1px solid var(--border-strong)",
              borderRadius: 8,
              boxShadow: "var(--shadow)",
            }}
          >
            {options.map((opt) => {
              const on = opt.value === choice;
              return (
                <div
                  key={opt.value}
                  onClick={() => {
                    setLang(opt.value);
                    pushSetting("vlx-lang", opt.value); // Also persist in the backend for cross-shell sharing.
                    setOpen(false);
                  }}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 6,
                    padding: "5px 8px",
                    borderRadius: 6,
                    cursor: "pointer",
                    fontSize: 11.5,
                    color: on ? "var(--accent)" : "var(--text)",
                    background: on ? "var(--accent-soft)" : "transparent",
                  }}
                  onMouseEnter={(e) => {
                    if (!on) e.currentTarget.style.background = "var(--bg-3, rgba(128,128,128,0.12))";
                  }}
                  onMouseLeave={(e) => {
                    if (!on) e.currentTarget.style.background = "transparent";
                  }}
                >
                  <span
                    style={{
                      flex: 1,
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {opt.label}
                  </span>
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

/** Custom default-shell dropdown containing the system default and detected shells. Hide the row
 * when detection is unavailable. New terminals without an explicit choice use this value; tree and
 * edit-form entry points may still override it per launch. */
function ShellSelect() {
  const t = useT();
  const defaultShell = useTermStore((s) => s.defaultShell);
  const setDefaultShell = useTermStore((s) => s.setDefaultShell);
  const [shells, setShells] = useState<ShellOption[]>([]);
  const [open, setOpen] = useState(false);
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

  const options: { value: string; label: string }[] = [
    { value: "", label: t("tree.shellSystemDefault") },
    ...shells.map((s) => ({ value: s.path, label: s.label })),
  ];
  const current = options.find((o) => o.value === defaultShell)?.label ?? defaultShell;

  return (
    <Field label={t("settings.defaultShell")}>
      <div style={{ position: "relative" }}>
        <button
          onClick={() => setOpen((o) => !o)}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 6,
            width: 160,
            height: 26,
            padding: "0 8px",
            background: "var(--bg-active)",
            color: "var(--text)",
            border: `1px solid ${open ? "var(--accent)" : "var(--border)"}`,
            borderRadius: 6,
            fontSize: 11.5,
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
          <Icons.chevD size={12} style={{ color: "var(--text-dim)", flex: "none" }} />
        </button>

        {open && (
          <>
            <div
              style={{ position: "fixed", inset: 0, zIndex: 1140 }}
              onMouseDown={() => setOpen(false)}
            />
            <div
              style={{
                position: "absolute",
                top: "calc(100% + 4px)",
                right: 0,
                zIndex: 1150,
                width: 172,
                maxHeight: 260,
                overflowY: "auto",
                padding: 4,
                background: "var(--bg-2)",
                border: "1px solid var(--border-strong)",
                borderRadius: 8,
                boxShadow: "var(--shadow)",
              }}
            >
              {options.map((opt) => {
                const on = opt.value === defaultShell;
                return (
                  <div
                    key={opt.value || "__system__"}
                    onClick={() => {
                      setDefaultShell(opt.value);
                      setOpen(false);
                    }}
                    style={{
                      display: "flex",
                      alignItems: "center",
                      gap: 6,
                      padding: "5px 8px",
                      borderRadius: 6,
                      cursor: "pointer",
                      fontSize: 11.5,
                      color: on ? "var(--accent)" : "var(--text)",
                      background: on ? "var(--accent-soft)" : "transparent",
                    }}
                    onMouseEnter={(e) => {
                      if (!on)
                        e.currentTarget.style.background = "var(--bg-3, rgba(128,128,128,0.12))";
                    }}
                    onMouseLeave={(e) => {
                      if (!on) e.currentTarget.style.background = "transparent";
                    }}
                  >
                    <span
                      style={{
                        flex: 1,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                    >
                      {opt.label}
                    </span>
                    {on && <Icons.check size={12} style={{ flex: "none" }} />}
                  </div>
                );
              })}
            </div>
          </>
        )}
      </div>
    </Field>
  );
}

/** Font picker with presets and a custom-name input. A null value uses the default monospace stack.
 * Like the other appearance dropdowns, it avoids native select rendering. */
function FontSelect({
  value,
  onChange,
}: {
  value: string | null;
  onChange: (v: string | null) => void;
}) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const isPreset = value != null && MONO_FONTS.includes(value);
  const display = value == null ? t("settings.fontDefault") : value;

  if (editing) {
    return (
      <input
        autoFocus
        value={draft}
        placeholder="Fira Code"
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          // In edit mode Escape exits editing only; stop propagation so it does not close the modal.
          if (e.key === "Enter") {
            onChange(draft.trim() || null);
            setEditing(false);
          } else if (e.key === "Escape") {
            e.stopPropagation();
            setEditing(false);
          }
        }}
        onBlur={() => {
          onChange(draft.trim() || null);
          setEditing(false);
        }}
        style={{
          width: 160,
          height: 26,
          padding: "0 8px",
          background: "var(--bg-active)",
          color: "var(--text)",
          border: "1px solid var(--accent)",
          borderRadius: 6,
          fontSize: 11.5,
          fontFamily: "inherit",
          outline: "none",
        }}
      />
    );
  }

  return (
    <div style={{ position: "relative" }}>
      <button
        onClick={() => setOpen((o) => !o)}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          width: 160,
          height: 26,
          padding: "0 8px",
          background: "var(--bg-active)",
          color: "var(--text)",
          border: `1px solid ${open ? "var(--accent)" : "var(--border)"}`,
          borderRadius: 6,
          fontSize: 11.5,
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
          {display}
        </span>
        <Icons.chevD size={12} style={{ color: "var(--text-dim)", flex: "none" }} />
      </button>

      {open && (
        <>
          <div
            style={{ position: "fixed", inset: 0, zIndex: 1140 }}
            onMouseDown={() => setOpen(false)}
          />
          <div
            style={{
              position: "absolute",
              top: "calc(100% + 4px)",
              right: 0,
              zIndex: 1150,
              width: 172,
              maxHeight: 260,
              overflowY: "auto",
              padding: 4,
              background: "var(--bg-2)",
              border: "1px solid var(--border-strong)",
              borderRadius: 8,
              boxShadow: "var(--shadow)",
            }}
          >
            {[
              {
                key: "__default",
                label: t("settings.fontDefault"),
                on: value == null,
                act: () => {
                  onChange(null);
                  setOpen(false);
                },
              },
              ...MONO_FONTS.map((f) => ({
                key: f,
                label: f,
                on: value === f,
                act: () => {
                  onChange(f);
                  setOpen(false);
                },
              })),
              {
                key: "__custom",
                label: t("settings.fontCustom"),
                on: value != null && !isPreset,
                act: () => {
                  setDraft(value && !isPreset ? value : "");
                  setOpen(false);
                  setEditing(true);
                },
              },
            ].map((opt) => (
              <div
                key={opt.key}
                onClick={opt.act}
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 6,
                  padding: "5px 8px",
                  borderRadius: 6,
                  cursor: "pointer",
                  fontSize: 11.5,
                  color: opt.on ? "var(--accent)" : "var(--text)",
                  background: opt.on ? "var(--accent-soft)" : "transparent",
                }}
                onMouseEnter={(e) => {
                  if (!opt.on)
                    e.currentTarget.style.background = "var(--bg-3, rgba(128,128,128,0.12))";
                }}
                onMouseLeave={(e) => {
                  if (!opt.on) e.currentTarget.style.background = "transparent";
                }}
              >
                <span
                  style={{
                    flex: 1,
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {opt.label}
                </span>
                {opt.on && <Icons.check size={12} style={{ flex: "none" }} />}
              </div>
            ))}
          </div>
        </>
      )}
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
}: {
  value: number | null;
  onChange: (v: number | null) => void;
  min: number;
  max: number;
  defaultValue: number;
  autoLabel?: string;
}) {
  const t = useT();
  const display = value == null ? (autoLabel ?? String(defaultValue)) : `${value}px`;
  const step = (d: number) => {
    const base = value ?? defaultValue;
    onChange(Math.max(min, Math.min(max, base + d)));
  };
  // UI size resets to null to follow density; terminal size resets to its explicit default.
  const reset = () => onChange(autoLabel != null ? null : defaultValue);
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
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


type Cat =
  | "appearance"
  | "terminal"
  | "behavior"
  | "advanced"
  | "agents"
  | "orchestration"
  | "shortcuts"
  | "general";

export function SettingsModal({ onClose }: { onClose: () => void }) {
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
  const usageRefreshSec = useTermStore((s) => s.usageRefreshSec);
  const setUsageRefreshSec = useTermStore((s) => s.setUsageRefreshSec);
  const soundEnabled = useTermStore((s) => s.soundEnabled);
  const toggleSound = useTermStore((s) => s.toggleSound);
  const uiFontFamily = useTermStore((s) => s.uiFontFamily);
  const uiFontSize = useTermStore((s) => s.uiFontSize);
  const termFontFamily = useTermStore((s) => s.termFontFamily);
  const termFontSize = useTermStore((s) => s.termFontSize);
  const setUiFontFamily = useTermStore((s) => s.setUiFontFamily);
  const setUiFontSize = useTermStore((s) => s.setUiFontSize);
  const setTermFontFamily = useTermStore((s) => s.setTermFontFamily);
  const setTermFontSize = useTermStore((s) => s.setTermFontSize);

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
    { key: "behavior", label: t("settings.catBehavior") },
    { key: "advanced", label: t("settings.catAdvanced") },
    { key: "agents", label: t("settings.catAgents") },
    { key: "orchestration", label: t("settings.catOrchestration") },
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
                  <FontSelect value={uiFontFamily} onChange={setUiFontFamily} />
                </Field>
                <Field label={t("settings.uiFontSize")}>
                  <FontSizeStepper
                    value={uiFontSize}
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
                <ShellSelect />
                <Field label={t("settings.termFont")}>
                  <FontSelect value={termFontFamily} onChange={setTermFontFamily} />
                </Field>
                <Field label={t("settings.termFontSize")}>
                  <FontSizeStepper
                    value={termFontSize}
                    onChange={(v) => setTermFontSize(v ?? 13)}
                    min={10}
                    max={24}
                    defaultValue={13}
                  />
                </Field>
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
                      ["canvas", "Canvas"],
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
              <>
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
                <Field label={t("settings.usageRefresh")}>
                  <Seg<string>
                    value={String(usageRefreshSec)}
                    options={[
                      ["0", t("common.off")],
                      ["30", "30s"],
                      ["60", "1m"],
                      ["120", "2m"],
                      ["300", "5m"],
                    ]}
                    onChange={(v) => setUsageRefreshSec(Number(v))}
                  />
                </Field>
                <CleanImagesField />
              </>
            )}

            {cat === "agents" && <AgentsPanel />}

            {cat === "orchestration" && <OrchestrationPanel />}

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
                            console.error("vlx skill failed:", e);
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
