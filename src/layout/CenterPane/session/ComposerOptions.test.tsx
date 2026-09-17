import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { ComposerOptionsButton, useComposerOptions } from "./ComposerOptions";
import { ControlChip } from "./controls";

function Harness({ mobile = true }: { mobile?: boolean }) {
  const options = useComposerOptions(mobile);
  const [draft, setDraft] = useState("Keep this draft");
  const [choice, setChoice] = useState("a");
  return <><button>Outside</button><div ref={options.ref}>
    <textarea aria-label="Message" value={draft} onChange={event => setDraft(event.target.value)} />
    {mobile && <ComposerOptionsButton expanded={options.expanded} onToggle={options.toggle} />}
    <div hidden={mobile && !options.expanded}>
      <ControlChip glyph={null} label={choice} title="Model" value={choice}
        options={[{ value: "a", label: "Model A" }, { value: "b", label: "Model B" }]} onPick={setChoice} />
    </div>
  </div></>;
}
afterEach(cleanup);
describe("compact composer options", () => {
  it("saves the current value when Set as default is checked without another selection", () => {
    const onKeepCurrent = vi.fn();
    render(<ControlChip glyph={null} label="Model A" title="Model" value="a"
      options={[{ value: "a", label: "Model A" }, { value: "b", label: "Model B" }]}
      onPick={vi.fn()} keepLabel="Set as default" onKeepCurrent={onKeepCurrent} />);
    fireEvent.click(screen.getByTitle("Model"));
    fireEvent.click(screen.getByRole("checkbox", { name: "Set as default" }));
    expect(onKeepCurrent).toHaveBeenCalledOnce();
    expect(onKeepCurrent).toHaveBeenCalledWith("a");
    expect(screen.getByRole("option", { name: "Model A" })).toBeTruthy();
  });

  it("opens and closes using the persistent button without focusing the input", () => {
    render(<Harness />);
    const toggle = screen.getByRole("button", { expanded: false });
    fireEvent.pointerDown(toggle);
    fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect(document.activeElement).not.toBe(screen.getByRole("textbox"));
    fireEvent.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
  });
  it("keeps options open through a chip selection and preserves the draft on outside click", () => {
    render(<Harness />);
    const toggle = screen.getByRole("button", { expanded: false });
    fireEvent.click(toggle);
    fireEvent.click(screen.getByTitle("Model"));
    fireEvent.pointerDown(screen.getByRole("option", { name: "Model B" }));
    fireEvent.mouseDown(screen.getByRole("option", { name: "Model B" }));
    expect(screen.getByTitle("Model").textContent).toBe("b");
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    fireEvent.pointerDown(screen.getByRole("button", { name: "Outside" }));
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("Keep this draft");
  });
  it("keeps options collapsed while typing and opens only through the toggle", () => {
    render(<Harness />);
    const input = screen.getByRole("textbox");
    input.focus();
    fireEvent.change(input, { target: { value: "Typing without options" } });
    const toggle = screen.getByRole("button", { expanded: false });
    expect(screen.getByTitle("Model").closest("[hidden]")).not.toBeNull();
    fireEvent.click(toggle);
    expect(document.activeElement).not.toBe(input);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    expect((input as HTMLTextAreaElement).value).toBe("Typing without options");
  });
  it("keeps desktop controls visible without a compact toggle", () => {
    render(<Harness mobile={false} />);
    expect(screen.getByTitle("Model")).toBeTruthy();
    expect(screen.queryByRole("button", { expanded: false })).toBeNull();
  });
});

describe("control chip filter", () => {
  const catalogue = [
    { value: "opencode/gpt-5.6-luna", label: "GPT-5.6 Luna", hint: "OpenCode Go" },
    { value: "opencode/deepseek-v4-flash", label: "DeepSeek V4 Flash", hint: "OpenCode Go" },
    { value: "opencode/deepseek-v4-pro", label: "DeepSeek V4 Pro", hint: "OpenCode Go" },
    { value: "opencode/glm-5.1", label: "GLM-5.1", hint: "OpenCode Go" },
    { value: "claude/haiku", label: "Claude Haiku", hint: "Anthropic" },
    ...Array.from({ length: 8 }, (_, index) => ({
      value: `other/model-${index}`,
      label: `Other Model ${index}`,
      hint: "Other Provider",
    })),
  ];

  function FilterHarness() {
    const [choice, setChoice] = useState("");
    return <ControlChip glyph={null} label={choice} title="Model" value={choice} options={catalogue}
      onPick={setChoice} filterPlaceholder="Filter models" />;
  }

  it("matches words from the name and the explanation together", () => {
    render(<FilterHarness />);
    fireEvent.click(screen.getByTitle("Model"));
    fireEvent.change(screen.getByPlaceholderText("Filter models"), { target: { value: "opencode go flash" } });
    expect(screen.getByRole("option", { name: /DeepSeek V4 Flash/ })).toBeTruthy();
    expect(screen.queryByRole("option", { name: /DeepSeek V4 Pro/ })).toBeNull();
  });

  it("shows the row matching the query and hides the rest", () => {
    render(<FilterHarness />);
    fireEvent.click(screen.getByTitle("Model"));
    fireEvent.change(screen.getByPlaceholderText("Filter models"), { target: { value: "anthropic" } });
    expect(screen.getAllByRole("option")).toHaveLength(1);
    expect(screen.getByRole("option", { name: /Claude Haiku/ })).toBeTruthy();
  });
});
