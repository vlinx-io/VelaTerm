//! Rows of the model and effort chips, shared by the conversation composer and the New Session tab so a
//! model reads the same before a session exists as inside it.

import Icons from "../../../components/Icons";
import type { useT } from "../../../i18n";
import type { SessionKind } from "../../../types";
import { kindIconEl } from "../../sessionViewers/sessionMeta";
import { LevelBar, type ChipOption } from "./controls";

/** Reasoning-effort values understood by the supported chat engines. */
export const EFFORTS = ["minimal", "low", "medium", "high", "xhigh", "max", "ultra"] as const;
/**
 * The level above the ladder: extra-high effort with the agent's dynamic workflow orchestration on top.
 * It runs at `xhigh`, so it is offered only by the models that reach that level.
 */
export const ULTRACODE = "ultracode" as const;
/**
 * Below the ladder: no extended thinking at all. Claude only; it is applied as a thinking-token cap of
 * zero rather than as a level, which is why the backend handles it apart from the others.
 */
export const THINKING_OFF = "off" as const;

/** What the menu offers for a model: the model's own ladder, plus ultracode wherever xhigh is reachable. */
export function levelsOf(ladder: readonly string[], kind: SessionKind): string[] {
  return kind === "claude" && ladder.includes("xhigh") ? [...ladder, ULTRACODE] : [...ladder];
}

/** Drop a leading "<name> · " from a model's description, where the name is already the row's label. */
export function withoutNamePrefix(description: string, label: string): string {
  const prefix = `${label} · `;
  return description.startsWith(prefix) ? description.slice(prefix.length) : description;
}

type T = ReturnType<typeof useT>;

/** A model as both catalogues describe it; the description and context flag are optional in launch lists. */
export interface ModelChipEntry { id: string; label: string; description?: string; largeContext?: boolean }

/**
 * The model menu: "follow the default" first, carrying the settings mark in the accent colour because it
 * names no model, then every model with the agent's own mark, 1M-context variants last.
 */
export function modelChipOptions(t: T, kind: SessionKind, models: readonly ModelChipEntry[], defaultLabel: string): ChipOption<string>[] {
  return [
    {
      value: "",
      label: defaultLabel,
      hint: t("chat.followModelDefaultHint"),
      glyph: (
        <span style={{ color: "var(--accent)", display: "inline-flex" }}>
          <Icons.sliders size={14} />
        </span>
      ),
    },
    // The catalogue's description opens with the model's own name, because it is also used where the name
    // is not already on screen. Here it is, one line above, so the menu drops the repeat.
    ...models.toSorted((a, b) => Number(!!a.largeContext) - Number(!!b.largeContext)).map((m) => ({
      value: m.id,
      label: m.label,
      hint: m.description ? withoutNamePrefix(m.description, m.label) || undefined : undefined,
      glyph: kindIconEl(kind, 14),
    })),
  ];
}

/** Display name of an effort level; a level outside the known ladder shows as reported. */
export function effortChipLabel(t: T, value: string): string {
  return (EFFORTS as readonly string[]).includes(value) || value === THINKING_OFF || value === ULTRACODE
    ? t(`chat.effort.${value}` as "chat.effort.high")
    : value;
}

/**
 * The effort menu from "auto" up: each level drawn as a bar filled to its place on the full ladder, so a
 * model that stops at high still shows high at its stable place. Auto gets an empty bar because it names
 * no level, and ultracode a full one because it runs at the top of the ladder. `extraLevels` keeps
 * levels a native catalogue reports outside the known ladder, listed after it.
 */
export function effortChipOptions(t: T, allowed: readonly string[], extraLevels = false): ChipOption<string>[] {
  const known = new Set<string>([...EFFORTS, ULTRACODE, THINKING_OFF]);
  return [
    { value: "", label: t("chat.effort.auto"), glyph: <LevelBar level={0} of={EFFORTS.length} /> },
    ...EFFORTS.filter((value) => allowed.includes(value)).map((value) => ({
      value,
      label: effortChipLabel(t, value),
      glyph: <LevelBar level={EFFORTS.indexOf(value) + 1} of={EFFORTS.length} />,
    })),
    ...(allowed.includes(ULTRACODE)
      ? [{ value: ULTRACODE, label: t("chat.effort.ultracode"), glyph: <LevelBar level={EFFORTS.length} of={EFFORTS.length} /> }]
      : []),
    ...(extraLevels ? allowed.filter((value) => !known.has(value)).map((value) => ({ value, label: value })) : []),
  ];
}
