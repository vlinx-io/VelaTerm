import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { setLang } from "../i18n";
import { platform } from "../platform";
import { ServiceRestartNotice } from "./ServiceRestartNotice";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("ServiceRestartNotice", () => {
  it("stays hidden when no background service restart occurred", async () => {
    setLang("en");
    vi.spyOn(platform.window, "takeServiceRestartNotice").mockResolvedValue(false);
    render(<ServiceRestartNotice />);
    await waitFor(() => expect(platform.window.takeServiceRestartNotice).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows a dismissible alert when the background service restarted unexpectedly", async () => {
    setLang("en");
    vi.spyOn(platform.window, "takeServiceRestartNotice").mockResolvedValue(true);
    render(<ServiceRestartNotice />);
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("background service restarted unexpectedly");
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
