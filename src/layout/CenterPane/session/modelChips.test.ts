import { expect, it } from "vitest";
import type { useT } from "../../../i18n";
import { THINKING_NONE, THINKING_OFF, effortChipLabel, effortChipOptions } from "./modelChips";

// The chips read their labels through i18n; the key itself is enough to see which row was drawn.
const t = ((key: string) => key) as unknown as ReturnType<typeof useT>;
const values = (allowed: readonly string[], extraLevels = false, includeOff?: boolean) =>
  effortChipOptions(t, allowed, extraLevels, includeOff).map((option) => option.value);

it("offers Grok's `none` as the row below the ladder, with Grok's own value", () => {
  const allowed = [THINKING_NONE, "low", "high", "max"];
  // Grok sends `none` back as a level, so the row has to carry that value rather than Claude's `off`.
  expect(values(allowed)).toEqual([THINKING_NONE, "", "low", "high", "max"]);
  expect(effortChipLabel(t, THINKING_NONE)).toBe("chat.effort.off");
  // A level the catalogue reports outside the known ladder is listed once, not twice.
  expect(values([THINKING_NONE, "high", "weird"], true)).toEqual([THINKING_NONE, "", "high", "weird"]);
});

it("keeps Claude's `off` switch as its own value", () => {
  // Claude's ladder names no `off`; the row is added explicitly and must stay `off`, not `none`.
  expect(values(["low", "high"], false, true)).toEqual([THINKING_OFF, "", "low", "high"]);
  expect(values(["low", "high"])).toEqual(["", "low", "high"]);
  expect(effortChipLabel(t, THINKING_OFF)).toBe("chat.effort.off");
});

it("draws the off row first and auto second, as the menu reads", () => {
  const rows = effortChipOptions(t, [THINKING_NONE, "high"]).map((option) => option.label);
  expect(rows).toEqual(["chat.effort.off", "chat.effort.auto", "chat.effort.high"]);
});
