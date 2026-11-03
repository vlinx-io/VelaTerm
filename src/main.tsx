//! Entry point. It only starts loading the application (bootstrap.tsx) as a separate chunk, the way DBX does:
//! the window already shows the startup skeleton from index.html, and everything else (styles, dictionaries,
//! React and the interface) arrives with that chunk instead of in the entry the page must parse first.

/** Session flag that limits the reload after a failed load to one attempt. */
const RETRY_KEY = "vlx-bootstrap-retry";

function retried(): boolean {
  try {
    return sessionStorage.getItem(RETRY_KEY) === "1";
  } catch {
    return true; // Without session storage a reload could repeat forever, so do not retry.
  }
}

import("./bootstrap").then(
  () => {
    try { sessionStorage.removeItem(RETRY_KEY); } catch { /* storage unavailable */ }
  },
  (error: unknown) => {
    console.error("[startup] failed to load the application", error);
    // A chunk can fail to load when an update replaced the assets under a page that was already open; one
    // reload picks up the current set.
    if (retried()) return;
    try { sessionStorage.setItem(RETRY_KEY, "1"); } catch { /* storage unavailable */ }
    location.reload();
  },
);
