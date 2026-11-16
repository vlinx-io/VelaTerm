//! Which session last wrote each artifact, learned from the tool calls conversation views render.
//!
//! The artifact header names that session and "Send to session" fills its composer. This lives in memory
//! only: it is a convenience, and a page opened before any session touched it simply has no writer.

type Listener = () => void;

const writers = new Map<string, string>();
const listeners = new Set<Listener>();

/** Record that `sessionId` wrote `path`. */
export function noteArtifactWriter(path: string, sessionId: string): void {
  if (writers.get(path) === sessionId) return;
  writers.set(path, sessionId);
  for (const l of listeners) l();
}

export function artifactWriter(path: string): string | undefined {
  return writers.get(path);
}

export function subscribeArtifactWriters(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** True for paths the artifact preview renders. */
export function isArtifactPath(path: string): boolean {
  return /\.html?$/i.test(path);
}

/** Event asking a conversation view to show a page in its side panel. */
export const ARTIFACT_OPEN_BESIDE_EVENT = "vlx:artifact-open-beside";

export interface ArtifactOpenBesideDetail {
  sessionId: string;
  path: string;
}

/**
 * Ask the mounted conversation of `sessionId` to show `path` beside it. Returns false when none took it (a
 * terminal session, a closed tab, a phone), and the caller opens a tab instead. A conversation in a
 * background tab still takes it and shows the panel when the reader switches back.
 */
export function openBesideSession(sessionId: string, path: string): boolean {
  const event = new CustomEvent<ArtifactOpenBesideDetail>(ARTIFACT_OPEN_BESIDE_EVENT, { detail: { sessionId, path }, cancelable: true });
  window.dispatchEvent(event);
  return event.defaultPrevented;
}

/** Event a conversation view listens for to put text into its composer without sending it. */
export const COMPOSER_INSERT_EVENT = "vlx:composer-insert";

export interface ComposerInsertDetail {
  sessionId: string;
  text: string;
}

export function insertIntoComposer(sessionId: string, text: string): void {
  window.dispatchEvent(new CustomEvent<ComposerInsertDetail>(COMPOSER_INSERT_EVENT, { detail: { sessionId, text } }));
}
