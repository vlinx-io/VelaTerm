import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  // uiZoom decides at import time whether native zoom is available; pretend to run inside the Electron shell,
  // whose zoom path goes through the optional preload bridge and needs no native import.
  (window as unknown as Record<string, unknown>).__VLX_ELECTRON__ = true;
});

import { ZoomSeg } from "./StatusBar";
import { getUiZoom, setUiZoom } from "../../platform/uiZoom";

const VALUE_LABEL = "Interface zoom percentage, 80 to 200";

function valueButton(): HTMLElement {
  return screen.getByRole("button", { name: VALUE_LABEL });
}

function openEditor(): HTMLInputElement {
  fireEvent.click(valueButton());
  return screen.getByRole("textbox", { name: VALUE_LABEL }) as HTMLInputElement;
}

beforeEach(() => {
  window.localStorage.clear();
  setUiZoom(1);
});
afterEach(cleanup);

describe("status-bar zoom input", () => {
  it("accepts a typed percentage with or without the sign", () => {
    render(<ZoomSeg />);
    expect(valueButton().textContent).toBe("100%");

    let input = openEditor();
    expect(input.value).toBe("100");
    // Entry selects the whole value so the first keystroke replaces it.
    expect(input.selectionStart).toBe(0);
    expect(input.selectionEnd).toBe(3);
    fireEvent.change(input, { target: { value: "125 %" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(getUiZoom()).toBe(1.25);
    expect(valueButton().textContent).toBe("125%");

    input = openEditor();
    fireEvent.change(input, { target: { value: "90" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(getUiZoom()).toBe(0.9);
    expect(valueButton().textContent).toBe("90%");
  });

  it("keeps the current level when the text is not a valid percentage", () => {
    render(<ZoomSeg />);
    const input = openEditor();
    fireEvent.change(input, { target: { value: "75" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(getUiZoom()).toBe(1);
    expect(valueButton().textContent).toBe("100%");

    fireEvent.click(valueButton());
    fireEvent.change(screen.getByRole("textbox", { name: VALUE_LABEL }), { target: { value: "abc" } });
    fireEvent.keyDown(screen.getByRole("textbox", { name: VALUE_LABEL }), { key: "Enter" });
    expect(getUiZoom()).toBe(1);
    expect(valueButton().textContent).toBe("100%");
  });

  it("commits on blur and discards the draft on Escape", () => {
    render(<ZoomSeg />);
    let input = openEditor();
    fireEvent.change(input, { target: { value: "150" } });
    fireEvent.blur(input);
    expect(getUiZoom()).toBe(1.5);
    expect(valueButton().textContent).toBe("150%");

    input = openEditor();
    fireEvent.change(input, { target: { value: "200" } });
    fireEvent.keyDown(input, { key: "Escape" });
    expect(getUiZoom()).toBe(1.5);
    expect(valueButton().textContent).toBe("150%");
  });
});
