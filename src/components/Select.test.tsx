import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import Select from "./Select";

const OPTIONS = [
  { value: "low", label: "low" },
  { value: "medium", label: "medium" },
  { value: "high", label: "high" },
];

/** Wrapper that actually holds the value, so a committed choice is visible on the trigger. */
function Harness({ initial = "low" }: { initial?: string }) {
  const [v, setV] = useState(initial);
  return <Select value={v} onChange={setV} options={OPTIONS} ariaLabel="effort" />;
}

describe("Select", () => {
  it("keeps a portaled menu outside clipping parents and preserves keyboard selection", () => {
    const onChange = vi.fn();
    const { container } = render(
      <div style={{ overflow: "hidden" }}>
        <Select menuPortal value="low" onChange={onChange} options={OPTIONS} ariaLabel="font" />
      </div>,
    );
    const trigger = screen.getByRole("combobox", { name: "font" });
    fireEvent.click(trigger);
    expect(container.querySelector('[role="listbox"]')).toBeNull();
    expect(screen.getByRole("listbox").parentElement).toBe(document.body);
    fireEvent.keyDown(trigger, { key: "End" });
    fireEvent.keyDown(trigger, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith("high");
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("opens on click, commits the clicked option and closes", () => {
    render(<Harness />);
    const trigger = screen.getByRole("combobox");
    expect(trigger.getAttribute("aria-expanded")).toBe("false");

    fireEvent.click(trigger);
    expect(trigger.getAttribute("aria-expanded")).toBe("true");

    fireEvent.click(screen.getByRole("option", { name: "high" }));
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    expect(trigger.textContent).toContain("high");
  });

  it("stays closed after a choice when wrapped in a label", () => {
    function Labeled() {
      const [v, setV] = useState("low");
      return <label>Model<Select width="100%" value={v} onChange={setV} options={OPTIONS} ariaLabel="model" /></label>;
    }
    render(<Labeled />);
    const trigger = screen.getByRole("combobox");
    fireEvent.click(trigger);
    fireEvent.click(screen.getByRole("option", { name: "high" }));
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(trigger.textContent).toContain("high");
  });

  it("arrows move the highlight and only Enter commits", () => {
    const onChange = vi.fn();
    render(<Select value="low" onChange={onChange} options={OPTIONS} ariaLabel="effort" />);
    const trigger = screen.getByRole("combobox");

    fireEvent.keyDown(trigger, { key: "ArrowDown" }); // opens
    fireEvent.keyDown(trigger, { key: "ArrowDown" }); // low -> medium
    expect(onChange).not.toHaveBeenCalled();

    fireEvent.keyDown(trigger, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith("medium");
  });

  it("skips disabled options and refuses to commit them", () => {
    const onChange = vi.fn();
    render(
      <Select
        value="low"
        onChange={onChange}
        options={[
          { value: "low", label: "low" },
          { value: "medium", label: "medium", disabled: true },
          { value: "high", label: "high" },
        ]}
        ariaLabel="effort"
      />,
    );
    const trigger = screen.getByRole("combobox");

    fireEvent.click(trigger);
    fireEvent.click(screen.getByRole("option", { name: "medium" }));
    expect(onChange).not.toHaveBeenCalled();

    fireEvent.keyDown(trigger, { key: "ArrowDown" }); // low -> high, stepping over medium
    fireEvent.keyDown(trigger, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith("high");
  });

  it("Escape closes without committing", () => {
    const onChange = vi.fn();
    render(<Select value="low" onChange={onChange} options={OPTIONS} ariaLabel="effort" />);
    const trigger = screen.getByRole("combobox");

    fireEvent.click(trigger);
    fireEvent.keyDown(trigger, { key: "Escape" });
    expect(trigger.getAttribute("aria-expanded")).toBe("false");
    expect(onChange).not.toHaveBeenCalled();
  });

  it("renders a leading caption alongside the trigger", () => {
    const onChange = vi.fn();
    render(
      <Select
        value="low"
        onChange={onChange}
        options={OPTIONS}
        leading="Address"
        ariaLabel="Address"
      />,
    );
    expect(screen.getByText("Address")).toBeDefined();

    const trigger = screen.getByRole("combobox");
    fireEvent.click(trigger);
    fireEvent.click(screen.getByRole("option", { name: "high" }));
    expect(onChange).toHaveBeenCalledWith("high");
  });

  it("filters rows by label when searchable and keeps arrow keys and Enter working", () => {
    const onChange = vi.fn();
    const onAction = vi.fn();
    render(<Select searchable searchPlaceholder="filter" noMatchLabel="none" value="low" onChange={onChange}
      options={[...OPTIONS, { value: "higher", label: "Higher" }]} ariaLabel="effort"
      actions={[{ id: "add", label: "Add", onSelect: onAction }]} />);
    fireEvent.click(screen.getByRole("combobox", { name: "effort" }));
    const box = screen.getByPlaceholderText("filter");
    expect(document.activeElement).toBe(box);
    fireEvent.change(box, { target: { value: "HIG" } });
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["high", "Higher", "Add"]);
    fireEvent.keyDown(box, { key: "ArrowDown" });
    fireEvent.keyDown(box, { key: "Enter" });
    expect(onChange).toHaveBeenCalledWith("higher");
    expect(screen.queryByRole("listbox")).toBeNull();

    fireEvent.click(screen.getByRole("combobox", { name: "effort" }));
    expect((screen.getByPlaceholderText("filter") as HTMLInputElement).value).toBe("");
    fireEvent.change(screen.getByPlaceholderText("filter"), { target: { value: "zzz" } });
    expect(screen.getByText("none")).toBeTruthy();
    fireEvent.keyDown(screen.getByPlaceholderText("filter"), { key: "ArrowDown" });
    fireEvent.keyDown(screen.getByPlaceholderText("filter"), { key: "Enter" });
    expect(onAction).toHaveBeenCalled();
  });

  it("opens the filter with the character typed on a closed trigger", () => {
    render(<Select searchable searchPlaceholder="filter" value="low" onChange={() => {}} options={OPTIONS} ariaLabel="effort" />);
    fireEvent.keyDown(screen.getByRole("combobox", { name: "effort" }), { key: "m" });
    expect((screen.getByPlaceholderText("filter") as HTMLInputElement).value).toBe("m");
    expect(screen.getAllByRole("option").map((o) => o.textContent)).toEqual(["medium"]);
  });

  it("shows the placeholder when no option matches the value", () => {
    render(
      <Select value="" onChange={() => {}} options={OPTIONS} placeholder="Select a branch" />,
    );
    expect(screen.getByRole("combobox").textContent).toContain("Select a branch");
  });
});

function MultipleHarness() {
  const [value, setValue] = useState(["low"]);
  return <Select multiple value={value} onChange={setValue} options={OPTIONS} menuPortal ariaLabel="types" selectAllLabel="Select All" clearLabel="Clear selection" placeholder="Choose types" />;
}

it("supports continuous multi-selection and bulk actions without closing the dropdown", () => {
  render(<MultipleHarness />);
  const trigger = screen.getByRole("combobox", { name: "types" });
  fireEvent.click(trigger);
  expect(screen.getByRole("listbox").getAttribute("aria-multiselectable")).toBe("true");
  fireEvent.click(screen.getByRole("option", { name: "medium" }));
  fireEvent.click(screen.getByRole("option", { name: "high" }));
  fireEvent.click(screen.getByRole("option", { name: "low" }));
  expect(trigger.textContent).toContain("medium, high");
  expect(trigger.getAttribute("aria-expanded")).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "Clear selection" }));
  expect(trigger.textContent).toContain("Choose types");
  fireEvent.click(screen.getByRole("button", { name: "Select All" }));
  expect(screen.getAllByRole("option", { selected: true })).toHaveLength(3);
  fireEvent.keyDown(trigger, { key: "Escape" });
  expect(screen.queryByRole("listbox")).toBeNull();
});

it("keeps keyboard focus on a toggled option and closes only the dropdown on Escape or Tab", () => {
  const parentEscape = vi.fn();
  render(<div onKeyDown={e => { if (e.key === "Escape") parentEscape(); }}><MultipleHarness /></div>);
  const trigger = screen.getByRole("combobox", { name: "types" });
  trigger.focus();
  fireEvent.keyDown(trigger, { key: "ArrowDown" });
  fireEvent.keyDown(trigger, { key: " " });
  expect(screen.getByRole("option", { name: "low" }).getAttribute("aria-selected")).toBe("false");
  fireEvent.keyDown(trigger, { key: " " });
  expect(screen.getByRole("option", { name: "low" }).getAttribute("aria-selected")).toBe("true");
  fireEvent.keyDown(trigger, { key: "a", ctrlKey: true });
  expect(screen.getAllByRole("option", { selected: true })).toHaveLength(3);
  fireEvent.keyDown(trigger, { key: "Escape" });
  expect(parentEscape).not.toHaveBeenCalled();
  fireEvent.click(trigger);
  fireEvent.keyDown(trigger, { key: "Tab" });
  expect(screen.queryByRole("listbox")).toBeNull();
});
