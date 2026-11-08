//! A native select-all over CodeMirror's rendered lines must become a whole-document selection,
//! while ordinary keyboard selections keep their range.

import { describe, expect, it } from "vitest";
import { isViewportSelectAll } from "./nativeSelectAll";

const DOC = 10_000;
const VIEWPORT = { from: 0, to: 4_200 };

describe("isViewportSelectAll", () => {
  it("recognizes a selection that covers exactly the rendered lines", () => {
    expect(isViewportSelectAll({ from: 0, to: 4_200 }, VIEWPORT, DOC)).toBe(true);
    // WebKit ends the range just past the last rendered line break.
    expect(isViewportSelectAll({ from: 0, to: 4_201 }, VIEWPORT, DOC)).toBe(true);
  });

  it("recognizes it when the user scrolled away from the start", () => {
    const viewport = { from: 3_000, to: 7_500 };
    expect(isViewportSelectAll({ from: 3_000, to: 7_500 }, viewport, DOC)).toBe(true);
    expect(isViewportSelectAll({ from: 2_999, to: 7_501 }, viewport, DOC)).toBe(true);
    expect(isViewportSelectAll({ from: 3_000, to: DOC }, { from: 3_000, to: DOC }, DOC)).toBe(true);
  });

  it("leaves partial selections alone", () => {
    expect(isViewportSelectAll({ from: 0, to: 1_200 }, VIEWPORT, DOC)).toBe(false);
    expect(isViewportSelectAll({ from: 500, to: 4_200 }, VIEWPORT, DOC)).toBe(false);
    expect(isViewportSelectAll({ from: 4_200, to: 4_200 }, VIEWPORT, DOC)).toBe(false);
  });

  it("does nothing when everything is already rendered or selected", () => {
    expect(isViewportSelectAll({ from: 0, to: 800 }, { from: 0, to: 800 }, 800)).toBe(false);
    expect(isViewportSelectAll({ from: 0, to: DOC }, VIEWPORT, DOC)).toBe(false);
  });
});
