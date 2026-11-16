//! Vela Skills details dialog, opened from the status-bar prompt. It lists what each bundled skill does
//! and offers install / later / don't-remind actions; see ipc/velaSkills.ts for the shared state.

import { useT } from "../i18n";
import {
  closeVelaSkillsModal,
  dismissVelaSkillsPrompt,
  installVelaSkills,
  useVelaSkillsState,
} from "../ipc/velaSkills";
import { Backdrop } from "./Backdrop";

/** Skill names stay untranslated because users type them as commands. */
const SKILLS = [
  ["vspawn", "skills.vspawn"],
  ["vspawn-tree", "skills.vspawnTree"],
  ["vopen", "skills.vopen"],
  ["vartifact", "skills.vartifact"],
  ["vrefer", "skills.vrefer"],
  ["vask", "skills.vask"],
  ["vsearch", "skills.vsearch"],
  ["vstat", "skills.vstat"],
  ["vtell", "skills.vtell"],
  ["vkb", "skills.vkb"],
  ["vapp", "skills.vapp"],
] as const;

export function VelaSkillsModal() {
  const t = useT();
  const { modalOpen, installing, error } = useVelaSkillsState();
  if (!modalOpen) return null;

  return (
    <Backdrop onClose={closeVelaSkillsModal}>
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          width: 520,
          maxWidth: "92vw",
          background: "var(--bg-app)",
          border: "1px solid var(--border)",
          borderRadius: 10,
          padding: 20,
          color: "var(--text-primary)",
          boxShadow: "0 12px 40px rgba(0,0,0,0.4)",
        }}
      >
        <div style={{ fontSize: 14, fontWeight: 600 }}>{t("skills.title")}</div>
        <div style={{ fontSize: 12, lineHeight: 1.55, color: "var(--text-dim)", marginTop: 6 }}>
          {t("skills.subtitle")}
        </div>

        <div
          style={{
            margin: "14px 0",
            padding: "10px 0",
            maxHeight: 320,
            overflowY: "auto",
            borderTop: "1px solid var(--border)",
            borderBottom: "1px solid var(--border)",
            display: "grid",
            gridTemplateColumns: "max-content 1fr",
            columnGap: 14,
            rowGap: 8,
            fontSize: 12.5,
            lineHeight: 1.5,
          }}
        >
          {SKILLS.map(([name, key]) => (
            <div key={name} style={{ display: "contents" }}>
              <code
                style={{
                  fontFamily: "var(--font-mono, monospace)",
                  fontSize: 12,
                  color: "var(--accent)",
                }}
              >
                {name}
              </code>
              <span style={{ color: "var(--text-dim)" }}>{t(key)}</span>
            </div>
          ))}
        </div>

        <div style={{ fontSize: 11.5, color: "var(--text-dim)", marginBottom: 12 }}>
          {t("skills.settingsHint")}
        </div>
        {error && (
          <div style={{ fontSize: 12, color: "var(--status-asking)", marginBottom: 12 }}>
            {t("skills.installFailed", error)}
          </div>
        )}

        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <button className="vlx-btn" onClick={dismissVelaSkillsPrompt} disabled={installing}>
            {t("skills.dontRemind")}
          </button>
          <div style={{ flex: 1 }} />
          <button className="vlx-btn" onClick={closeVelaSkillsModal} disabled={installing}>
            {t("skills.later")}
          </button>
          <button
            className="vlx-btn vlx-btn-primary"
            onClick={() => void installVelaSkills().catch(() => {})}
            disabled={installing}
          >
            {installing ? t("skills.installing") : t("skills.install")}
          </button>
        </div>
      </div>
    </Backdrop>
  );
}
