/** Browsers defer undelivered resize notifications to the next frame; this is not a script exception. */
export function isDeferredResizeNotification(event: Pick<ErrorEvent, "message" | "error">): boolean {
  return event.error == null && (
    event.message === "ResizeObserver loop completed with undelivered notifications." ||
    event.message === "ResizeObserver loop limit exceeded"
  );
}
