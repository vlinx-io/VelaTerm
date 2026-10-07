import { afterEach, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { useTermStore } from "../store/termStore";
import { PanelRail } from "./PanelRail";

afterEach(() => {
  cleanup();
  useTermStore.setState({ leftCollapsed: false, rightCollapsed: false });
});

it("restores the sidebar from the left rail", () => {
  useTermStore.setState({ leftCollapsed: true });
  render(<PanelRail side="left" />);
  fireEvent.click(screen.getByRole("button", { name: "Show sidebar" }));
  expect(useTermStore.getState().leftCollapsed).toBe(false);
});

it("restores the info panel from the right rail", () => {
  useTermStore.setState({ rightCollapsed: true });
  render(<PanelRail side="right" />);
  fireEvent.click(screen.getByRole("button", { name: "Show info panel" }));
  expect(useTermStore.getState().rightCollapsed).toBe(false);
});
