
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const { state } = vi.hoisted(() => ({
  state: {
    orchestrationProfiles: {} as Record<string, Record<string, unknown>>,
    orchestration: {
      maxDescendants: 10,
      maxParallel: 4,
      maxDepth: 2,
      requireConfirmationAbove: 6,
      autoApprove: false,
      defaultTimeoutSecs: 1800,
      worktreeCopyPatterns: ["docs/plans/**"],
    },
    setOrchestrationProfile: vi.fn(),
    setOrchestrationLimits: vi.fn(),
  },
}));

vi.mock("../../i18n", () => ({
  useT: () => (key: string) => key,
}));
vi.mock("../../store/termStore", () => ({
  useTermStore: (selector: (s: object) => unknown) => selector(state),
}));

import { OrchestrationPanel } from "./settingsPanels";

afterEach(() => {
  cleanup();
  state.setOrchestrationProfile.mockClear();
  state.setOrchestrationLimits.mockClear();
});

describe("OrchestrationPanel launch values", () => {
  it("hides model and effort for an agent whose launch flags ignore them", () => {
    state.orchestrationProfiles = { docs: { agent: "cursor", model: "fable" } };
    render(<OrchestrationPanel />);
    expect(screen.queryByText("settings.orchModel")).toBeNull();
    expect(screen.queryByText("settings.orchEffort")).toBeNull();
  });

  it("keeps a stored model that the selected agent does not list", () => {
    state.orchestrationProfiles = { critical: { agent: "claude", model: "sol" } };
    render(<OrchestrationPanel />);
    expect(screen.getByDisplayValue("sol")).toBeTruthy();
  });

  it("shows a listed model through the dropdown instead of the free-text input", () => {
    state.orchestrationProfiles = { critical: { agent: "claude", model: "fable" } };
    render(<OrchestrationPanel />);
    expect(screen.getByText("fable")).toBeTruthy();
    expect(screen.queryByDisplayValue("fable")).toBeNull();
  });

  it("uses a dropdown for permission mode and persists the selection", () => {
    state.orchestrationProfiles = {
      critical: { agent: "claude", permissionMode: "default" },
    };
    render(<OrchestrationPanel />);

    fireEvent.click(screen.getByRole("button", { name: "settings.orchPermissionMode" }));
    const skipOptions = screen.getAllByText("settings.orchPermissionSkip");
    fireEvent.click(skipOptions[skipOptions.length - 1]);

    expect(state.setOrchestrationProfile).toHaveBeenCalledWith("critical", {
      permissionMode: "skip",
    });
  });

  it("offers the parent permission mode for cross-agent workers", () => {
    state.orchestrationProfiles = {
      critical: { agent: "codex", permissionMode: "default" },
    };
    render(<OrchestrationPanel />);

    fireEvent.click(screen.getByRole("button", { name: "settings.orchPermissionMode" }));
    const inheritOptions = screen.getAllByText("settings.orchPermissionInherit");
    fireEvent.click(inheritOptions[inheritOptions.length - 1]);

    expect(state.setOrchestrationProfile).toHaveBeenCalledWith("critical", {
      permissionMode: "inherit",
    });
  });

  it("warns when the selected profile skips confirmations", () => {
    state.orchestrationProfiles = {
      critical: { agent: "claude", permissionMode: "skip" },
    };
    render(<OrchestrationPanel />);

    expect(screen.getByText("settings.orchPermissionSkipWarning")).toBeTruthy();
  });

  it("does not warn when the selected profile uses default permissions", () => {
    state.orchestrationProfiles = {
      critical: { agent: "claude", permissionMode: "default" },
    };
    render(<OrchestrationPanel />);

    expect(screen.queryByText("settings.orchPermissionSkipWarning")).toBeNull();
  });
});

describe("OrchestrationPanel descriptions", () => {
  it("shows and persists the selected profile description", () => {
    state.orchestrationProfiles = {
      frontend: {
        description: "Use for UI components and browser interactions.",
        agent: "claude",
      },
    };
    render(<OrchestrationPanel />);

    const description = screen.getByRole("textbox", {
      name: "settings.orchDescription",
    }) as HTMLTextAreaElement;
    expect(description.value).toBe("Use for UI components and browser interactions.");
    const profileEditor = description.closest(".orch-profile-editor");
    const descriptionBlock = description.closest(".orch-description-block");
    expect(profileEditor).not.toBeNull();
    expect(descriptionBlock).not.toBeNull();
    expect(descriptionBlock?.querySelector("label")?.htmlFor).toBe(description.id);
    expect(descriptionBlock?.lastElementChild).toBe(description);
    expect(description.placeholder).toBe("settings.orchDescriptionPlaceholder");
    expect(
      screen
        .getByText("settings.orchProfilesHint")
        .compareDocumentPosition(screen.getByRole("button", { name: "settings.orchProfile" })) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      descriptionBlock!.compareDocumentPosition(screen.getByText("resume.agentType")) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();

    fireEvent.change(description, { target: { value: "Use for responsive UI work." } });
    fireEvent.blur(description);
    expect(state.setOrchestrationProfile).toHaveBeenCalledWith("frontend", {
      description: "Use for responsive UI work.",
    });
  });
});

describe("OrchestrationPanel profile creation", () => {
  it("reveals the profile name field from the Add New dropdown option", () => {
    state.orchestrationProfiles = {};
    render(<OrchestrationPanel />);

    fireEvent.click(screen.getByRole("button", { name: "settings.orchProfile" }));
    const addOptions = screen.getAllByText("settings.orchAddNew");
    fireEvent.click(addOptions[addOptions.length - 1]);

    expect(screen.getByPlaceholderText("settings.orchNewProfile")).toBeTruthy();
  });

  it("does not leave a second profile input when the temporary editor closes", () => {
    state.orchestrationProfiles = {};
    render(<OrchestrationPanel />);

    fireEvent.click(screen.getByRole("button", { name: "settings.orchProfile" }));
    const addOptions = screen.getAllByText("settings.orchAddNew");
    fireEvent.click(addOptions[addOptions.length - 1]);
    const input = screen.getByPlaceholderText("settings.orchNewProfile");
    fireEvent.blur(input);

    expect(screen.queryByPlaceholderText("settings.orchNewProfile")).toBeNull();
  });

  it("renames the selected profile and keeps its configuration", () => {
    state.orchestrationProfiles = {
      docs: { agent: "codex", model: "gpt-5.6-luna", worktree: false },
    };
    render(<OrchestrationPanel />);

    fireEvent.click(screen.getByRole("button", { name: "common.rename" }));
    const input = screen.getByDisplayValue("docs");
    fireEvent.change(input, { target: { value: "review" } });
    fireEvent.click(screen.getByRole("button", { name: "common.save" }));

    expect(state.setOrchestrationProfile).toHaveBeenNthCalledWith(1, "docs", null);
    expect(state.setOrchestrationProfile).toHaveBeenNthCalledWith(2, "review", {
      agent: "codex",
      model: "gpt-5.6-luna",
      worktree: false,
    });
  });
});

describe("OrchestrationPanel limits", () => {
  it("shows auto-approval disabled by default and persists changes", () => {
    state.orchestrationProfiles = {};
    render(<OrchestrationPanel />);
    const control = screen.getByRole("button", { name: "common.off" });
    fireEvent.click(control);

    const on = screen.getByRole("button", { name: "common.on" });
    fireEvent.click(on);
    expect(state.setOrchestrationLimits).toHaveBeenCalledWith({ autoApprove: true });
  });

  it("restores the stored value instead of persisting a non-numeric entry", () => {
    state.orchestrationProfiles = {};
    render(<OrchestrationPanel />);
    const field = screen.getByDisplayValue("10") as HTMLInputElement;
    fireEvent.change(field, { target: { value: "many" } });
    fireEvent.blur(field);
    expect(state.setOrchestrationLimits).not.toHaveBeenCalled();
    expect(field.value).toBe("10");
  });

  it("clamps a below-minimum entry to one and persists it", () => {
    state.orchestrationProfiles = {};
    render(<OrchestrationPanel />);
    const field = screen.getByDisplayValue("4") as HTMLInputElement;
    fireEvent.change(field, { target: { value: "0" } });
    fireEvent.blur(field);
    expect(state.setOrchestrationLimits).toHaveBeenCalledWith({ maxParallel: 1 });
  });

  it("drops empty lines when committing worktree copy patterns", () => {
    state.orchestrationProfiles = {};
    render(<OrchestrationPanel />);
    const patterns = screen.getByDisplayValue("docs/plans/**") as HTMLTextAreaElement;
    fireEvent.change(patterns, { target: { value: "docs/plans/**\n\n  .env.local  \n" } });
    fireEvent.blur(patterns);
    expect(state.setOrchestrationLimits).toHaveBeenCalledWith({
      worktreeCopyPatterns: ["docs/plans/**", ".env.local"],
    });
  });
});
