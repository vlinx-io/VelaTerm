//! The terminal renderer default is a deliberate product decision (DOM, decided 2026-08-16);
//! a silent flip inside an unrelated commit must fail here.

import { beforeEach, describe, expect, it } from "vitest";
import { loadSettings, SETTINGS_KEY } from "./settings";

describe("terminal renderer setting", () => {
  beforeEach(() => localStorage.clear());

  it("defaults to the DOM renderer when nothing is stored", () => {
    expect(loadSettings().termRenderer).toBe("dom");
  });

  it("preserves a stored renderer choice", () => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ termRenderer: "canvas" }));
    expect(loadSettings().termRenderer).toBe("canvas");
  });

  it("migrates the legacy gpuRender boolean only when no renderer is stored", () => {
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ gpuRender: true }));
    expect(loadSettings().termRenderer).toBe("webgl");
    localStorage.setItem(SETTINGS_KEY, JSON.stringify({ gpuRender: false }));
    expect(loadSettings().termRenderer).toBe("dom");
    localStorage.setItem(
      SETTINGS_KEY,
      JSON.stringify({ gpuRender: true, termRenderer: "canvas" }),
    );
    expect(loadSettings().termRenderer).toBe("canvas");
  });
});
