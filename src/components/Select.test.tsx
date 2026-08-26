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

  it("shows the placeholder when no option matches the value", () => {
    render(
      <Select value="" onChange={() => {}} options={OPTIONS} placeholder="Select a branch" />,
    );
    expect(screen.getByRole("combobox").textContent).toContain("Select a branch");
  });
});
