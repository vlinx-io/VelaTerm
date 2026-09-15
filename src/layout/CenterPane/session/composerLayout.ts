//! Pure layout arithmetic for the composer toolbar: which chips the user turned on, and how many of them
//! fit beside the message before the rest fold into the More row. No React and no DOM, so the toolbar
//! can inject measured widths and the tests can inject made-up ones.

import type { ReactNode } from "react";
import type { ComposerChipId } from "../../../store/settings";

/** One chip the pane offers for the current session, keyed for the user's preference list. */
export interface ComposerChip {
  id: ComposerChipId;
  node: ReactNode;
}

/**
 * Splits the chips that exist for this session by the user's preference. `enabled` follows the order of
 * `inline` and skips ids the pane did not offer, so an enabled chip that is unavailable for this agent
 * or engine state is never rendered as an empty slot. `hidden` keeps the pane's own order.
 */
export function arrangeComposerChips(chips: ComposerChip[], inline: ComposerChipId[]): {
  enabled: ComposerChip[];
  hidden: ComposerChip[];
} {
  const byId = new Map(chips.map((chip) => [chip.id, chip]));
  const enabled: ComposerChip[] = [];
  for (const id of inline) {
    const chip = byId.get(id);
    if (chip && !enabled.includes(chip)) enabled.push(chip);
  }
  const hidden = chips.filter((chip) => !enabled.includes(chip));
  return { enabled, hidden };
}

/**
 * Decides how many of the enabled chips stay inline. All of them, when they fit without the More toggle;
 * otherwise as many leading chips as fit next to the toggle, which may be none. A width of 0 means "not
 * measured yet" and counts as fitting, so the chip gets rendered once and measured before the next pass.
 */
export function partitionComposerChips(
  widths: number[],
  available: number,
  moreWidth: number,
  gap: number,
): { inlineCount: number; overflow: boolean } {
  // Unmeasured items take neither width nor a gap, so a row nobody has measured yet always fits.
  const rowWidth = (count: number, extra: number) => {
    let total = 0;
    let items = 0;
    for (const width of [...widths.slice(0, count), extra]) {
      if (width <= 0) continue;
      total += width;
      items += 1;
    }
    return total + Math.max(0, items - 1) * gap;
  };
  if (rowWidth(widths.length, 0) <= available) return { inlineCount: widths.length, overflow: false };
  let inlineCount = 0;
  while (inlineCount < widths.length && rowWidth(inlineCount + 1, moreWidth) <= available) inlineCount += 1;
  return { inlineCount, overflow: true };
}
