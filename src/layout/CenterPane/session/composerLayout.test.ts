import { describe, expect, it } from "vitest";
import { arrangeComposerChips, partitionComposerChips, type ComposerChip } from "./composerLayout";

const chip = (id: ComposerChip["id"]): ComposerChip => ({ id, node: id });

describe("partitionComposerChips", () => {
  it("keeps every chip inline without a More toggle when the row fits exactly", () => {
    // 40 + 60 + 50 plus two gaps of 6 is 162.
    expect(partitionComposerChips([40, 60, 50], 162, 70, 6)).toEqual({ inlineCount: 3, overflow: false });
  });

  it("folds everything when not even the first chip fits beside the toggle", () => {
    expect(partitionComposerChips([100, 60], 120, 70, 6)).toEqual({ inlineCount: 0, overflow: true });
  });

  it("lets the toggle push out the chip before the one that no longer fits", () => {
    // 40 + 60 + 6 = 106 fits alone; with the 70 wide toggle and its gap (182) the second chip has to go too.
    expect(partitionComposerChips([40, 60, 50], 120, 70, 6)).toEqual({ inlineCount: 1, overflow: true });
    // A narrower toggle keeps the second chip: 40 + 60 + 20 + 12 = 132.
    expect(partitionComposerChips([40, 60, 50], 132, 20, 6)).toEqual({ inlineCount: 2, overflow: true });
  });

  it("brings chips back as the row grows", () => {
    const widths = [40, 60, 50];
    expect(partitionComposerChips(widths, 100, 70, 6).inlineCount).toBe(0);
    expect(partitionComposerChips(widths, 130, 70, 6)).toEqual({ inlineCount: 1, overflow: true });
    // The toggle is wider than the last chip: one pixel short of fitting all three still folds two.
    expect(partitionComposerChips(widths, 161, 70, 6)).toEqual({ inlineCount: 1, overflow: true });
    expect(partitionComposerChips(widths, 162, 70, 6)).toEqual({ inlineCount: 3, overflow: false });
  });

  it("treats an unmeasured chip as fitting so it gets rendered and measured", () => {
    expect(partitionComposerChips([40, 0, 0], 40, 0, 6)).toEqual({ inlineCount: 3, overflow: false });
    expect(partitionComposerChips([40, 0, 90], 46, 0, 6)).toEqual({ inlineCount: 2, overflow: true });
  });

  it("handles an empty row and a zero-width container", () => {
    expect(partitionComposerChips([], 0, 70, 6)).toEqual({ inlineCount: 0, overflow: false });
    expect(partitionComposerChips([0, 0], 0, 0, 6)).toEqual({ inlineCount: 2, overflow: false });
  });
});

describe("arrangeComposerChips", () => {
  const offered = [chip("model"), chip("effort"), chip("permission"), chip("account")];

  it("follows the preference order and skips enabled chips the pane did not offer", () => {
    const { enabled, hidden } = arrangeComposerChips(offered, ["permission", "mcp", "model", "permission"]);
    expect(enabled.map((c) => c.id)).toEqual(["permission", "model"]);
    expect(hidden.map((c) => c.id)).toEqual(["effort", "account"]);
  });

  it("hides everything when nothing is enabled", () => {
    const { enabled, hidden } = arrangeComposerChips(offered, []);
    expect(enabled).toEqual([]);
    expect(hidden.map((c) => c.id)).toEqual(["model", "effort", "permission", "account"]);
  });
});
