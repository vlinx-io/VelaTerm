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
const observed = new Set<Element>();
const unobserve = vi.fn((el: Element) => observed.delete(el));
const disconnect = vi.fn(() => observed.clear());

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
    observe(el: Element) { observed.add(el); }
    unobserve = unobserve;
    disconnect = disconnect;
  });
});
afterEach(() => {
  cleanup();
  observers.clear();
  observed.clear();
  unobserve.mockClear();
  disconnect.mockClear();
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
  it("never renders a chip that is off, not even behind More", () => {
    widths = { row: 400, more: 70, model: 80, effort: 60, permission: 90 };
    render(<Harness inline={["model", "effort", "permission"]} />);
    expect(screen.getByRole("button", { name: "Model" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Effort" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Permission" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Send" })).toBeTruthy();
    expect(screen.getByText("Pending permission")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
    expect(document.querySelector('[data-chip="serviceTier"]')).toBeNull();
    expect(document.querySelector(".sv-controls-secondary")).toBeNull();
  });

  it("orders inline chips by the preference, not by the pane", () => {
    widths = { row: 400, more: 70, model: 80, effort: 60, permission: 90 };
    render(<Harness inline={["permission", "model"]} />);
    const names = [...document.querySelectorAll(".sv-controls-primary .sv-chip-slot button")].map((el) => el.textContent);
    expect(names).toEqual(["Permission", "Model"]);
  });

  it("folds only the enabled chips that do not fit behind a More toggle", () => {
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
    expect(folded).toEqual(["effort", "permission"]);
    fireEvent.click(more);
    expect(screen.queryByRole("button", { name: "Effort" })).toBeNull();
    expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("Keep this message");
  });

  it("closes selectors before the settings row and folds on Escape or outside click without stopping work", () => {
    // Not even the first enabled chip fits, so the toggle stands alone and everything is folded.
    widths = { row: 60, more: 70, model: 80 };
    render(<Harness inline={["model", "serviceTier"]} />);
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

  it("brings a folded chip back and drops More once nothing is folded", () => {
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
    widths.row = 130;
    act(() => observers.forEach((callback) => callback()));
    expect(screen.queryByRole("button", { name: "Effort" })).toBeNull();
    expect(screen.getByRole("button", { name: "More" }).getAttribute("aria-expanded")).toBe("false");
  });

  it("renders no chip and no More when every chip is off", () => {
    widths = { row: 400, more: 70, model: 80 };
    const { rerender } = render(<Harness inline={[]} />);
    expect(document.querySelectorAll(".sv-chip-slot")).toHaveLength(0);
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
    expect(document.querySelector(".sv-more-toggle")).toBeNull();
    expect(screen.getByRole("button", { name: "Send" })).toBeTruthy();
    rerender(<ComposerToolbar chips={[]} inline={[]} mobile={false} actions={<button>Send</button>} status={null} />);
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
    expect(document.querySelector(".sv-controls-secondary")).toBeNull();
  });

  it("remeasures folded chips after their labels or fonts change, without opening the row", () => {
    widths = { row: 120, more: 30, model: 80, effort: 120, permission: 150, serviceTier: 80 };
    render(<Harness inline={["model", "effort", "permission", "serviceTier"]} />);
    expect(screen.queryByRole("button", { name: "Effort" })).toBeNull();
    widths.effort = 12;
    widths.row = 134;
    act(() => observers.forEach(callback => callback()));
    expect(screen.getByRole("button", { name: "Effort" })).toBeTruthy();
    widths.effort = 100;
    act(() => observers.forEach(callback => callback()));
    expect(screen.queryByRole("button", { name: "Effort" })).toBeNull();
    expect(document.querySelector(".sv-controls-secondary")?.hasAttribute("inert")).toBe(true);
  });

  it("drops More only when all available chips fit inline and releases observer targets", () => {
    widths = { row: 120, more: 30, model: 80, effort: 60, permission: 90, serviceTier: 70 };
    const { rerender, unmount } = render(<Harness inline={["model", "effort", "permission", "serviceTier"]} />);
    fireEvent.click(screen.getByRole("button", { name: "More" }));
    widths.row = 500;
    act(() => observers.forEach(callback => callback()));
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
    expect(screen.getByRole("button", { name: "Effort" })).toBeTruthy();
    expect(document.querySelector(".sv-controls-secondary")).toBeNull();
    expect(unobserve).toHaveBeenCalled();
    const before = observed.size;
    rerender(<Harness inline={["model", "effort", "permission", "serviceTier"]} />);
    expect(observed.size).toBe(before);
    unmount();
    expect(disconnect).toHaveBeenCalledOnce();
    expect(observed.size).toBe(0);
  });

  it("wraps the enabled chips on mobile without measuring, a toggle or the chips that are off", () => {
    widths = { row: 10, more: 70, model: 80, effort: 60, permission: 90 };
    render(<Harness mobile inline={["model", "effort"]} />);
    expect(screen.queryByRole("button", { name: "More" })).toBeNull();
    expect(screen.getByRole("button", { name: "Model" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Effort" })).toBeTruthy();
    expect(document.querySelector(".sv-controls-secondary")).toBeNull();
    expect([...document.querySelectorAll(".sv-chip-slot")].map((el) => (el as HTMLElement).dataset.chip)).toEqual(["model", "effort"]);
  });
});
