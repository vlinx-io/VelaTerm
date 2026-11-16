import { describe, expect, it } from "vitest";
import { normalizeUiZoom, parseUiZoomPercent, stepUiZoom, UI_ZOOM_MAX, UI_ZOOM_MIN } from "./uiZoom";

describe("ui zoom", () => {
  it("clamps and rounds stored values, falling back to 1 for junk", () => {
    expect(normalizeUiZoom("1.25")).toBe(1.25);
    expect(normalizeUiZoom(0.1)).toBe(UI_ZOOM_MIN);
    expect(normalizeUiZoom(9)).toBe(UI_ZOOM_MAX);
    expect(normalizeUiZoom(1.234)).toBe(1.23);
    expect(normalizeUiZoom("abc")).toBe(1);
    expect(normalizeUiZoom(Number.NaN)).toBe(1);
  });

  it("steps to the neighbouring presets from the default level", () => {
    expect(stepUiZoom(1)).toBe(1.1);
    expect(stepUiZoom(-1)).toBe(0.9);
  });

  it("parses a typed percentage with an optional percent sign", () => {
    expect(parseUiZoomPercent("125")).toBe(1.25);
    expect(parseUiZoomPercent("125%")).toBe(1.25);
    expect(parseUiZoomPercent("125 %")).toBe(1.25);
    expect(parseUiZoomPercent(" 80% ")).toBe(UI_ZOOM_MIN);
    expect(parseUiZoomPercent("200%")).toBe(UI_ZOOM_MAX);
    expect(parseUiZoomPercent("０１２５％")).toBe(1.25);
  });

  it("rejects text outside the supported whole-percentage range", () => {
    expect(parseUiZoomPercent("75")).toBeNull();
    expect(parseUiZoomPercent("250%")).toBeNull();
    expect(parseUiZoomPercent("110.5")).toBeNull();
    expect(parseUiZoomPercent("100%%")).toBeNull();
    expect(parseUiZoomPercent("-100")).toBeNull();
    expect(parseUiZoomPercent("1e2")).toBeNull();
    expect(parseUiZoomPercent("")).toBeNull();
    expect(parseUiZoomPercent("  %  ")).toBeNull();
    expect(parseUiZoomPercent("abc")).toBeNull();
  });
});
