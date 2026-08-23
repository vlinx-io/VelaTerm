//! Icon fallback for agent presets: a preset without an uploaded image must still show something, and a
//! session created from a deleted preset must fall back rather than render nothing.

import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import { PresetIcon, sessionIconEl } from "./agentPresetIcon";
import type { AgentPreset } from "../types";

const base: AgentPreset = {
  id: "p1",
  name: "DeepSeek",
  baseKind: "claude",
  execPath: "/opt/bin/claude-deepseek",
  agentArgs: null,
  permissionMode: null,
  icon: null,
  sortOrder: 0,
  createdAt: 0,
};

describe("agent preset icons", () => {
  it("falls back to the base kind's icon when no image was uploaded", () => {
    const { container } = render(<PresetIcon preset={base} />);
    expect(container.querySelector("img")).toBeNull();
    // The built-in icons render as inline SVG.
    expect(container.querySelector("svg")).not.toBeNull();
  });

  it("renders the uploaded image when there is one", () => {
    const withIcon = { ...base, icon: "data:image/png;base64,AAAA" };
    const { container } = render(<PresetIcon preset={withIcon} />);
    expect(container.querySelector("img")?.getAttribute("src")).toBe(
      "data:image/png;base64,AAAA",
    );
  });

  it("shows a session's preset icon, and the kind's icon once that preset is gone", () => {
    const withIcon = { ...base, icon: "data:image/png;base64,BBBB" };
    const session = { kind: "claude" as const, agentPresetId: "p1" };

    const found = render(<>{sessionIconEl(session, [withIcon])}</>);
    expect(found.container.querySelector("img")).not.toBeNull();

    // Deleting the preset leaves a dangling ID by design; the session keeps launching and falls back here.
    const missing = render(<>{sessionIconEl(session, [])}</>);
    expect(missing.container.querySelector("img")).toBeNull();
    expect(missing.container.querySelector("svg")).not.toBeNull();
  });
});
