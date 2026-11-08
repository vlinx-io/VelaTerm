/** Recoverable history locations remain scoped to the conversation whose links created them. */
export function historyLocation(sessionId: string) {
  const query = new URLSearchParams(window.location.search);
  // Locations are row ids of one conversation; without its session id they would be applied to any other.
  if (query.get("chatSession") !== sessionId) return {};
  return { before: query.get("chatBefore") ?? undefined, after: query.get("chatAfter") ?? undefined, anchor: query.getAll("chatChild")[0] ?? query.get("chatRow") ?? undefined };
}

export function historyPageURL(sessionId: string, direction: "before" | "after", boundary?: string) {
  const url = new URL(window.location.href);
  for (const key of ["chatBefore", "chatAfter", "chatRow", "chatField", "chatPart", "chatFiles", "chatChild"]) url.searchParams.delete(key);
  url.searchParams.set("chatSession", sessionId);
  url.searchParams.set("session", sessionId);
  if (boundary) url.searchParams.set(direction === "before" ? "chatBefore" : "chatAfter", boundary);
  return url.href;
}
