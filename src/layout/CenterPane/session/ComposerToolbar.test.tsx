import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { ComposerToolbar } from "./ComposerToolbar";
import { ControlChip } from "./controls";
import type { ComposerChip } from "./composerLayout";
import type { ComposerChipId } from "../../../store/settings";

/** Widths by chip id, plus the toolbar row and the More toggle; jsdom itself reports 0 for everything. */
let widths: Record<string, number> = {};
const observers = new Set<() => void>();

beforeEach(() => {
  widths = {};
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    const key = this.classList.contains("sv-controls-primary") ? "row"
      : this.classList.contains("sv-more-toggle") ? "more"
      : this.classList.contains("sv-chip-slot") ? this.dataset.chip ?? ""
      : "";
    // Slots inside the folded row report no width, like a display:none element would.
    const width = this.closest("[hidden]") ? 0 : widths[key] ?? 0;
    return { width, height: 26, top: 0, left: 0, right: width, bottom: 26, x: 0, y: 0, toJSON: () => ({}) } as DOMRect;
  });
  vi.stubGlobal("ResizeObserver", class {
    constructor(callback: () => void) { observers.add(callback); }
    observe() {}
    unobserve() {}
    disconnect() {}
  });
});
afterEach(() => {
  cleanup();
  observers.clear();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

function Harness({ mobile = false, inline }: { mobile?: boolean; inline: ComposerChipId[] }) {
  const [choice, setChoice] = useState("standard");
  const [stopped, setStopped] = useState(false);
  const chips: ComposerChip[] = [
    { id: "model", node: <button>Model</button> },
    { id: "effort", node: <button>Effort</button> },
    { id: "permission", node: <button>Permission</button> },
    { id: "serviceTier", node: <ControlChip glyph={null} label={choice} title="Speed" value={choice}
      options={[{ value: "standard", label: "Standard" }, { value: "fast", label: "Fast" }]} onPick={setChoice} /> },
  ];
  return <div className="sv-composer" onKeyDown={event => {
    if (event.key === "Escape" && !event.currentTarget.querySelector('[role="option"]')) setStopped(true);
  }}>
    <textarea aria-label="Draft" defaultValue="Keep this message" />
    <ComposerToolbar mobile={mobile} chips={chips} inline={inline} actions={<button>Send</button>}
      status={<span>{stopped ? "Stopped" : "Pending permission"}</span>} />
  </div>;
}

describe("composer toolbar", () => {
  it("shows every enabled chip without a More toggle when the row is wide enough", () => {
    widths = { row: 400, more: 70, model: 80, effort: 60, permission: 90 };
    render(<Harness inline={["model", "effort", "permission"]} />);
    expect(screen.getByRole("button", { name: "Model" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Effort" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Permission" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Send" })).toBeTruthy();
    expect(screen.getByText("Pending permission")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
    // The disabled chip is neither inline nor in a folded row.
    expect(screen.queryByTitle("Speed")).toBeNull();
    expect(document.querySelector(".sv-controls-secondary")).toBeNull();
  });

  it("orders inline chips by the preference, not by the pane", () => {
    widths = { row: 400, more: 70, model: 80, effort: 60, permission: 90 };
    render(<Harness inline={["permission", "model"]} />);
    const names = [...document.querySelectorAll(".sv-controls-primary .sv-chip-slot button")].map((el) => el.textContent);
    expect(names).toEqual(["Permission", "Model"]);
  });

  it("folds the chips that do not fit together with the disabled ones behind a More toggle", () => {
    // 80 + 6 + 60 = 146 fits; adding the 90 wide permission chip or the toggle does not.
    widths = { row: 220, more: 70, model: 80, effort: 60, permission: 90 };
    render(<Harness inline={["model", "effort", "permission"]} />);
    expect(screen.getByRole("button", { name: "Model" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Effort" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Permission" })).toBeNull();
    expect(screen.queryByRole("button", { name: "standard" })).toBeNull();
    const more = screen.getByRole("button", { name: "More" });
    expect(more.getAttribute("aria-expanded")).toBe("false");
    fireEvent.click(more);
    expect(more.getAttribute("aria-expanded")).toBe("true");
    const row = document.getElementById(more.getAttribute("aria-controls")!)!;
    expect(row.hidden).toBe(false);
    const folded = [...row.querySelectorAll(".sv-chip-slot")].map((el) => (el as HTMLElement).dataset.chip);
    expect(folded).toEqual(["effort", "permission", "serviceTier"]);
    fireEvent.click(more);
    expect(screen.queryByRole("button", { name: "Effort" })).toBeNull();
    expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("Keep this message");
  });

  it("closes selectors before the settings row and folds on Escape or outside click without stopping work", () => {
    // Not even the one enabled chip fits, so the toggle stands alone and everything is folded.
    widths = { row: 60, more: 70, model: 80 };
    render(<Harness inline={["model"]} />);
    const more = screen.getByRole("button", { name: "More" });
    fireEvent.click(more);
    const speed = screen.getByTitle("Speed");
    fireEvent.click(speed);
    fireEvent.keyDown(speed, { key: "Escape" });
    expect(screen.queryByRole("option")).toBeNull();
    expect(more.getAttribute("aria-expanded")).toBe("true");
    fireEvent.click(speed);
    fireEvent.mouseDown(screen.getByRole("option", { name: "Fast" }));
    fireEvent.keyDown(speed, { key: "Escape" });
    expect(more.getAttribute("aria-expanded")).toBe("false");
    expect(document.activeElement).toBe(more);
    expect(screen.queryByText("Stopped")).toBeNull();
    fireEvent.click(more);
    expect(screen.getByRole("button", { name: "fast" })).toBeTruthy();
    fireEvent.pointerDown(screen.getByRole("textbox"));
    expect(more.getAttribute("aria-expanded")).toBe("false");
  });

  it("brings a folded chip back and drops the toggle when the row grows", () => {
    // 80 + 60 + 6 = 146 does not fit in 130; the model chip plus the 30 wide toggle (116) does.
    widths = { row: 130, more: 30, model: 80, effort: 60 };
    render(<Harness inline={["model", "effort"]} />);
    expect(screen.queryByRole("button", { name: "Effort" })).toBeNull();
    const more = screen.getByRole("button", { name: "More" });
    fireEvent.click(more);
    expect(more.getAttribute("aria-expanded")).toBe("true");
    widths.row = 300;
    act(() => observers.forEach((callback) => callback()));
    expect(screen.getByRole("button", { name: "Effort" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
    expect(document.querySelector(".sv-controls-secondary")).toBeNull();
    widths.row = 130;
    act(() => observers.forEach((callback) => callback()));
    expect(screen.queryByRole("button", { name: "Effort" })).toBeNull();
    expect(screen.getByRole("button", { name: "More" }).getAttribute("aria-expanded")).toBe("false");
  });

  it("keeps both rows visible on mobile without measuring or a toggle", () => {
    widths = { row: 10, more: 70, model: 80, effort: 60, permission: 90 };
    render(<Harness mobile inline={["model", "effort"]} />);
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
    expect(screen.getByRole("button", { name: "Model" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Effort" })).toBeTruthy();
    const row = document.querySelector<HTMLElement>(".sv-controls-secondary")!;
    expect(row.hidden).toBe(false);
    expect([...row.querySelectorAll(".sv-chip-slot")].map((el) => (el as HTMLElement).dataset.chip)).toEqual(["permission", "serviceTier"]);
  });
});
