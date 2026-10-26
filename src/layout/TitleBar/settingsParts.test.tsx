//! Coverage for the optional Field hint: settings whose effect is not obvious carry an info glyph
//! explaining them, and rows without a hint stay exactly as they were.

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { Field } from "./settingsParts";

afterEach(cleanup);

describe("Field hint", () => {
  it("exposes the hint as a tooltip and an accessible name", () => {
    render(
      <Field label="Background limit" hint="How many sessions keep running in the background.">
        <button>32</button>
      </Field>,
    );
    const note = screen.getByRole("note");
    expect(note.getAttribute("title")).toBe("How many sessions keep running in the background.");
    expect(note.getAttribute("aria-label")).toBe(
      "How many sessions keep running in the background.",
    );
  });

  it("renders no glyph when no hint is given", () => {
    render(
      <Field label="Tabs">
        <button>single</button>
      </Field>,
    );
    expect(screen.queryByRole("note")).toBeNull();
    expect(screen.getByText("Tabs")).toBeTruthy();
  });
});
