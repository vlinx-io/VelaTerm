//! Shared normalization for built-in browser address-bar input.
//!
//! TypeScript equivalent of `src-tauri/src/browser.rs::normalize_url`; rules and tests must remain
//! aligned with Rust. Built-in navigation is security-sensitive, so HTTPS completion, search-query
//! conversion, and unsafe-scheme rejection are centralized here.
//!
//! - Electron normalizes in BrowserCapability before sending a clean URL to main.cjs's WebContentsView;
//!   main.cjs retains a scheme allowlist as defense in depth.
//! - Tauri currently normalizes in Rust. Separate tests keep both implementations aligned until the
//!   frontend can own normalization for both paths.
//!
//! Like Rust, successful input returns a normalized URL and invalid input throws Error for callers
//! to catch. Address-bar navigation failures are logged only.

/** Allowed navigation schemes: HTTP, HTTPS, and about for blank pages; reject file and all others. */
function schemeAllowed(scheme: string): boolean {
  const s = scheme.toLowerCase();
  return s === "http" || s === "https" || s === "about";
}

/** Scheme character set matching Rust, used to recognize explicit schemes. */
function isSchemeChars(scheme: string): boolean {
  return /^[A-Za-z0-9+\-.]+$/.test(scheme);
}

/**
 * Normalize address-bar input, ported from browser.rs::normalize_url:
 * - reject empty input;
 * - preserve case-insensitive `about:blank`;
 * - allow only HTTP, HTTPS, and about for explicit `://` schemes;
 * - prepend HTTPS to whitespace-free domain-like input (dot in first segment or localhost);
 * - treat everything else as a Google search query.
 */
export function normalizeBrowserUrl(input: string): string {
  const trimmed = input.trim();
  if (!trimmed) throw new Error("Empty address");
  if (trimmed.toLowerCase() === "about:blank") return "about:blank";

  // Parse explicit `xxx://` schemes and apply the allowlist.
  const pos = trimmed.indexOf("://");
  if (pos > 0 && isSchemeChars(trimmed.slice(0, pos))) {
    let url: URL;
    try {
      url = new URL(trimmed);
    } catch (e) {
      throw new Error(`Invalid URL: ${String(e)}`);
    }
    const scheme = url.protocol.replace(/:$/, "");
    if (!schemeAllowed(scheme)) throw new Error(`Scheme not allowed: ${scheme}`);
    return url.toString();
  }

  // Prepend HTTPS to domain-like input: dot/localhost in the first segment and no whitespace.
  const noSpace = !/\s/.test(trimmed);
  const hostPart = trimmed.split(/[/:]/, 1)[0] ?? "";
  const domainLike = noSpace && (hostPart.includes(".") || hostPart === "localhost");
  if (domainLike) {
    let url: URL;
    try {
      url = new URL(`https://${trimmed}`);
    } catch (e) {
      throw new Error(`Invalid URL: ${String(e)}`);
    }
    const scheme = url.protocol.replace(/:$/, "");
    if (!schemeAllowed(scheme)) throw new Error(`Scheme not allowed: ${scheme}`);
    return url.toString();
  }

  // Search query; v1 fixes Google, with engine selection left for a future setting.
  const search = new URL("https://www.google.com/search");
  search.searchParams.set("q", trimmed);
  return search.toString();
}

const CLAUDE_ARTIFACT_HOSTS = new Set(["claude.ai", "preview.claude.ai"]);
const CLAUDE_ARTIFACT_PATH = /^\/(?:code\/)?artifact\/[^/]+/;

/**
 * Whether a URL is a Claude artifact page. These pages rely on the Claude sign-in held by the user's
 * own browser, so they open there instead of in a built-in tab, whose cookies are isolated.
 */
export function isClaudeArtifactUrl(input: string): boolean {
  let url: URL;
  try {
    url = new URL(input);
  } catch {
    return false;
  }
  return url.protocol === "https:" && CLAUDE_ARTIFACT_HOSTS.has(url.host) && CLAUDE_ARTIFACT_PATH.test(url.pathname);
}
