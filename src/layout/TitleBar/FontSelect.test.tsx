import { act, cleanup, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { FontCatalog } from "../../platform/types";

const { catalogCall } = vi.hoisted(() => ({ catalogCall: vi.fn() }));
vi.mock("../../platform", () => ({ platform: { fonts: { catalog: catalogCall } } }));

import { setLang } from "../../i18n";
import { FontSelect, isKnownFontSelection, useFontCatalog } from "./FontSelect";

const catalog: FontCatalog = { families: ["JetBrains Mono", "User Installed Font"], systemFontsAvailable: true };
beforeEach(() => { setLang("en"); catalogCall.mockReset(); });
afterEach(cleanup);

it("offers catalog families and retains a saved custom value", () => {
  render(<FontSelect value="Saved Font" onChange={vi.fn()} label="Font" fonts={{ catalog, loading: false }} />);
  fireEvent.click(screen.getByRole("combobox", { name: "Font" }));
  expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual([
    "Default", "Saved Font", "JetBrains Mono", "User Installed Font",
  ]);
  expect(screen.queryByText("Consolas")).toBeNull();
  expect(screen.queryByText("Not installed on this device")).toBeNull();
  expect(screen.getByRole("status").textContent).toBe("Unable to confirm whether this font is available.");
});

it("filters families as the user types and commits the highlighted match", () => {
  const change = vi.fn();
  render(<FontSelect value={null} onChange={change} label="Font" fonts={{ catalog, loading: false }} />);
  fireEvent.click(screen.getByRole("combobox", { name: "Font" }));
  const search = screen.getByRole("textbox", { name: "Search fonts…" });
  fireEvent.change(search, { target: { value: "user" } });
  expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual([
    "User Installed Font", "Use “user”",
  ]);
  fireEvent.keyDown(search, { key: "Enter" });
  expect(change).toHaveBeenCalledExactlyOnceWith("User Installed Font");
});

it("accepts an installed font without measuring canvas text", () => {
  const measure = vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(() => { throw new Error("Canvas must not be used"); });
  try {
    render(<FontSelect value="User Installed Font" onChange={vi.fn()} label="Font" fonts={{ catalog, loading: false }} />);
    expect(screen.queryByRole("status")).toBeNull();
    expect(measure).not.toHaveBeenCalled();
  } finally { measure.mockRestore(); }
});

it("keeps bundled fonts and typed names when system enumeration is unavailable", () => {
  render(<FontSelect value="JetBrains Mono" onChange={vi.fn()} label="Font" fonts={{ catalog: { families: ["JetBrains Mono"], systemFontsAvailable: false }, loading: false }} />);
  expect(screen.getByRole("status").textContent).toContain("Unable to list system fonts");
  fireEvent.click(screen.getByRole("combobox"));
  expect(screen.getByRole("option", { name: "JetBrains Mono" })).toBeTruthy();
  fireEvent.change(screen.getByRole("textbox", { name: "Search fonts…" }), { target: { value: "Offline Font" } });
  expect(screen.getByRole("option", { name: "Use “Offline Font”" })).toBeTruthy();
});

it("commits a typed fallback list as-is and cancels on Escape", () => {
  const change = vi.fn();
  render(<FontSelect value="User Installed Font" onChange={change} label="Font" fonts={{ catalog, loading: false }} />);
  const open = () => {
    fireEvent.click(screen.getByRole("combobox"));
    return screen.getByRole("textbox", { name: "Search fonts…" });
  };
  let search = open();
  fireEvent.change(search, { target: { value: '"User Installed Font", monospace' } });
  fireEvent.click(screen.getByRole("option", { name: /Use “"User Installed Font", monospace”/ }));
  expect(change).toHaveBeenCalledExactlyOnceWith('"User Installed Font", monospace');
  change.mockClear();
  search = open();
  fireEvent.change(search, { target: { value: "Canceled Font" } });
  fireEvent.keyDown(search, { key: "Escape" });
  fireEvent.blur(search);
  expect(change).not.toHaveBeenCalled();
});

it("recognizes quoted families and CSS fallbacks without inventing missing-font verdicts", () => {
  expect(isKnownFontSelection('"user installed font"', catalog)).toBe(true);
  expect(isKnownFontSelection('Missing, "User Installed Font"', catalog)).toBe(true);
  expect(isKnownFontSelection('Missing, monospace', catalog)).toBe(true);
  expect(isKnownFontSelection('"monospace"', catalog)).toBe(false);
  expect(isKnownFontSelection('"Font, With Comma"', { families: ["Font, With Comma"], systemFontsAvailable: true })).toBe(true);
  expect(isKnownFontSelection('"unterminated, monospace', catalog)).toBe(false);
  expect(isKnownFontSelection("Unknown Font", catalog)).toBe(false);
});

it("refreshes after focus, ignores stale responses and clears stale data on failure", async () => {
  let first!: (value: FontCatalog) => void;
  catalogCall.mockReturnValueOnce(new Promise<FontCatalog>((resolve) => { first = resolve; }));
  const { result } = renderHook(useFontCatalog);
  expect(result.current.loading).toBe(true);
  catalogCall.mockResolvedValueOnce(catalog);
  await act(async () => { window.dispatchEvent(new Event("focus")); });
  expect(result.current.catalog).toEqual(catalog);
  await act(async () => { first({ families: ["Old Font"], systemFontsAvailable: true }); });
  expect(result.current.catalog).toEqual(catalog);
  catalogCall.mockRejectedValueOnce(new Error("Unavailable"));
  await act(async () => { window.dispatchEvent(new Event("focus")); });
  expect(result.current).toEqual({ catalog: null, loading: false });
});
