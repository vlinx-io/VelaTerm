import { expect, it } from "vitest";
import { isDeferredResizeNotification } from "./resizeObserverError";

it("recognizes browser notifications that defer resize delivery", () => {
  expect(isDeferredResizeNotification({ message: "ResizeObserver loop completed with undelivered notifications.", error: null })).toBe(true);
  expect(isDeferredResizeNotification({ message: "ResizeObserver loop limit exceeded", error: undefined })).toBe(true);
});

it("preserves actual script errors, including exceptions with the same message", () => {
  const message = "ResizeObserver loop completed with undelivered notifications.";
  expect(isDeferredResizeNotification({ message, error: new Error(message) })).toBe(false);
  expect(isDeferredResizeNotification({ message: "ResizeObserver is not defined", error: null })).toBe(false);
  expect(isDeferredResizeNotification({ message: "Unexpected failure", error: null })).toBe(false);
});
