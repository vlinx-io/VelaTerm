// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { COMPOSER_FEEDBACK_HEIGHT, COMPOSER_MIN_HEIGHT, composerHasFeedbackRoom, useShortPane } from "./composerHeight";

afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

let notify = () => {};
function Pane() {
  const [ref, short] = useShortPane();
  return <div ref={ref} data-testid="pane">{short ? "short" : "tall"}</div>;
}
const resize = (height: number) => {
  Object.defineProperty(screen.getByTestId("pane"), "clientHeight", { configurable: true, value: height });
  act(() => notify());
};

it("reports a short pane and ignores the zero height of a hidden one", () => {
  vi.stubGlobal("ResizeObserver", class { constructor(callback: () => void) { notify = callback; } observe() {} disconnect() {} });
  render(<Pane />);
  resize(800);
  expect(screen.getByTestId("pane").textContent).toBe("tall");
  resize(300);
  expect(screen.getByTestId("pane").textContent).toBe("short");
  resize(0);
  expect(screen.getByTestId("pane").textContent).toBe("short");
  resize(320);
  expect(screen.getByTestId("pane").textContent).toBe("tall");
});

it("gives the feedback line room only where the text box can spare its height", () => {
  expect(composerHasFeedbackRoom(null, false)).toBe(true);
  expect(composerHasFeedbackRoom(null, true)).toBe(false);
  expect(composerHasFeedbackRoom(COMPOSER_MIN_HEIGHT + COMPOSER_FEEDBACK_HEIGHT, true)).toBe(true);
  expect(composerHasFeedbackRoom(COMPOSER_MIN_HEIGHT + COMPOSER_FEEDBACK_HEIGHT - 1, false)).toBe(false);
});
