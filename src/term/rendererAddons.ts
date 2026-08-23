//! On-demand loading of xterm's Canvas and WebGL renderer addons.
//!
//! Why these are not imported normally: together they weigh about 200 KB in the entry bundle, yet DOM
//! is the default renderer and the only one verified to work under Tauri's WKWebView (canvas
//! mismeasures cell width against the configured font; WebGL loses contexts and corrupts glyph
//! atlases in hidden views). So the overwhelmingly common path pays for two addons it never
//! constructs. Loading them only when a session actually selects that renderer removes them from the
//! entry bundle entirely.
//!
//! Constructors are cached after the first load, which lets callers that must stay synchronous — such
//! as rebuilding a stale WebGL context during reveal — read the already-resolved constructor.

import type { CanvasAddon } from "@xterm/addon-canvas";
import type { WebglAddon } from "@xterm/addon-webgl";

type WebglCtor = typeof WebglAddon;
type CanvasCtor = typeof CanvasAddon;

let webglCtor: WebglCtor | null = null;
let canvasCtor: CanvasCtor | null = null;

/**
 * Load the WebGL addon constructor, returning null when the chunk cannot be fetched. Null is not
 * exceptional: every caller already treats an unavailable WebGL renderer as "stay on DOM".
 */
export async function loadWebglCtor(): Promise<WebglCtor | null> {
  if (webglCtor) return webglCtor;
  try {
    webglCtor = (await import("@xterm/addon-webgl")).WebglAddon;
    return webglCtor;
  } catch {
    return null;
  }
}

/** Load the Canvas addon constructor; null means the caller should stay on the DOM renderer. */
export async function loadCanvasCtor(): Promise<CanvasCtor | null> {
  if (canvasCtor) return canvasCtor;
  try {
    canvasCtor = (await import("@xterm/addon-canvas")).CanvasAddon;
    return canvasCtor;
  } catch {
    return null;
  }
}

/**
 * The WebGL constructor if a previous load already resolved it, otherwise null. For synchronous call
 * sites that only run after WebGL was successfully attached once, where awaiting would restructure
 * surrounding logic for no benefit.
 */
export function peekWebglCtor(): WebglCtor | null {
  return webglCtor;
}
