//! Reports that the clipboard source stopped responding, so a paste was abandoned instead of freezing the app.
//! Native paste and context-menu paste both raise it; App mounts one notice that renders it.

/** Rejection from a clipboard read abandoned because the source is not responding. */
export class ClipboardUnresponsiveError extends Error {
  constructor() {
    super("Clipboard source is not responding");
    this.name = "ClipboardUnresponsiveError";
  }
}

const listeners = new Set<() => void>();

export function reportClipboardUnresponsive(): void {
  listeners.forEach((listener) => listener());
}

export function onClipboardUnresponsive(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
