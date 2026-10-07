//! Workaround for delayed IME punctuation in WebKit, including macOS WKWebView.
//!
//! ## Background
//! macOS WebKit has long-standing IME event bugs (WebKit Bugzilla #164324 / #165004). Full-width punctuation
//! committed **directly without candidate selection** neither emits `compositionend` nor receives a timely
//! `input`; the input is delayed and ordered before its keydown. xterm therefore sends the character only after
//! the **next input**, appearing as a long punctuation lag. Chinese text entered through pinyin and candidate
//! selection is unaffected, and Chromium handles identical frontend code correctly by processing directly
//! committed punctuation synchronously as keydown. The fault is therefore in WebKit, not xterm, this project,
//! or the user's IME.
//!
//! ## Only directly committed full-width punctuation uses the input-event workaround
//! Safari/WebKit traces captured in June 2026 confirm that lag occurs only for directly committed punctuation.
//! An isolated punctuation character emits only `input` (`inputType=insertText`, `isComposing=false`) and no
//! compositionend, with input/keydown reversed and xterm's state machine disrupted. `onInput` handles that path.
//! Candidate-based commits emit compositionend normally and do not lag, so compositionend remains entirely under
//! xterm's native composition handling. An earlier compositionend workaround was removed because it addressed no
//! real lag and incorrectly intercepted composed non-Han input such as Japanese kana and Korean Hangul.
//!
//! ## Approach, using only public APIs
//! Write the reliable, immediate text from `input` directly to the PTY, bypassing WebKit's late reordered event.
//! xterm later emits the same text through `onData`; a short-lived deduplication table consumes that duplicate.
//! Because xterm emits exactly the committed event `data`, exact string matching does not affect other input.
//!
//! ## Half-width pass-through symbols use keyup
//! WKWebView tests show that in Chinese and similar modes, half-width symbols passed through without conversion or
//! candidate selection receive `input` only on the **next keypress**. The final symbol can remain stuck indefinitely,
//! and continuous typing stays one character behind. The timely `keyup` contains the committed character in `key`,
//! so send it immediately and let the deduplication table consume xterm's later copy.
//!
//! Four checks constrain the workaround:
//! - **keyCode===229** identifies an IME-processed keydown across languages. English input uses the real code and
//!   is never recorded.
//! - The keyup `key` must be a half-width ASCII symbol. Full-width conversions normally fall outside ASCII and
//!   use the input path; letters, numbers, and spaces are excluded by `isPassthroughSymbol`. Some Shift-modified
//!   keys can still report their half-width source, so the next two checks are also required.
//! - A timely input clears the keydown marker before keyup, preventing a duplicate. Only pass-through symbols with
//!   delayed input retain the marker and are sent from keyup.
//! - A late reordered keydown is not marked. For full-width punctuation, input arrives first; marking the later
//!   keydown would cause keyup to send the half-width source as well. Any IME keydown arriving within
//!   `REORDERED_KEYDOWN_MS` of an immediate input commit is treated as that reordered event and ignored.
//!
//! ## Deliberately narrow scope
//! - Enable only on WebKit (WKWebView, WebKitGTK, Safari), never Chromium/WebView2 where event ordering is correct.
//! - `onInput` accepts only non-composing `insertText` data made entirely of full-width/CJK punctuation. Normal
//!   ASCII, composing Chinese, Japanese kana, and Korean Hangul remain untouched.
//!
//! The workaround is isolated and removable by deleting this file and its three connections in usePtySession.ts.

import type { Terminal } from "@xterm/xterm";

/**
 * Whether the current WebView is WebKit rather than Chromium. Matches WKWebView, WebKitGTK, and Safari, but
 * excludes WebView2, Chrome, and Edge.
 */
export function isWebkitEngine(): boolean {
  if (typeof navigator === "undefined") return false;
  const ua = navigator.userAgent;
  return /AppleWebKit/.test(ua) && !/Chrome|Chromium|Edg\//.test(ua);
}

/**
 * Whether text consists entirely of full-width/CJK punctuation, the category WebKit commits directly through
 * input events. Every character must lie in a punctuation/symbol block so normal ASCII and half-width punctuation
 * never match. Covered ranges are General Punctuation U+2010-U+205E, CJK Symbols and Punctuation U+3000-U+303F,
 * and Halfwidth and Fullwidth Forms U+FF00-U+FFEF.
 *
 * The full-width block also contains digits and Latin letters, which must be explicitly excluded. Treating them
 * as punctuation would add them to the deduplication table and could consume the user's next identical character,
 * making it appear that a number or letter must be typed twice.
 */
function isFullwidthPunct(s: string): boolean {
  if (!s) return false;
  for (const ch of s) {
    const c = ch.codePointAt(0)!;
    // Exclude full-width digits and Latin letters even though they lie within U+FF00-U+FFEF.
    if (
      (c >= 0xff10 && c <= 0xff19) ||
      (c >= 0xff21 && c <= 0xff3a) ||
      (c >= 0xff41 && c <= 0xff5a)
    ) {
      return false;
    }
    const ok =
      (c >= 0x2010 && c <= 0x205e) ||
      (c >= 0x3000 && c <= 0x303f) ||
      (c >= 0xff00 && c <= 0xffef);
    if (!ok) return false;
  }
  return true;
}

/**
 * Whether text is one visible ASCII symbol of the kind an IME passes through without composition or candidate
 * selection. WebKit delays their input event until the next keypress, so timely `keyup` handles them instead.
 *
 * Deliberately accepts only one visible ASCII symbol (0x21-0x7E, excluding letters and digits). Letters form
 * pinyin/romaji and must not be sent during keyup; digits and spaces select candidates. Punctuation converted to
 * full-width falls outside this range and naturally remains on the onInput path.
 */
function isPassthroughSymbol(s: string): boolean {
  if (s.length !== 1) return false;
  const c = s.codePointAt(0)!;
  if (c < 0x21 || c > 0x7e) return false; // Visible ASCII only; exclude space, controls, full-width, and CJK.
  if (c >= 0x30 && c <= 0x39) return false; // 0-9
  if (c >= 0x41 && c <= 0x5a) return false; // A-Z
  if (c >= 0x61 && c <= 0x7a) return false; // a-z
  return true;
}

/**
 * The half-width ASCII character a full-width form stands for, or "" when the text is not full-width ASCII.
 *
 * One physical key can reach the PTY through two channels: `keyup` reports the key's half-width source
 * (`/`) and the IME's conversion arrives later through `input` (`／`). Comparing the channels needs a form
 * they share, so full-width ASCII (U+FF01-U+FF5E, which excludes the full-width space U+3000) maps back to
 * its source; every other character, including CJK punctuation, has no half-width counterpart.
 */
function halfwidthAscii(s: string): string {
  if (!s) return "";
  let out = "";
  for (const ch of s) {
    const c = ch.codePointAt(0)!;
    if (c < 0xff01 || c > 0xff5e) return "";
    out += String.fromCodePoint(c - 0xfee0);
  }
  return out;
}

/** Backlog bound for the deduplication table, as a memory safety valve rather than a policy.
 *
 *  Every workaround commit is owed exactly one later duplicate, so an entry must not be discarded while that
 *  duplicate is still on its way. Two earlier bounds were wrong for that reason: a five-second expiry dropped
 *  entries whenever the next keystroke came later than the window (the duplicate arrives with that keystroke,
 *  so pausing mid-word before retyping lost it), and a 16-entry cap evicted live registrations during a burst
 *  of punctuation. Both sent the character to the PTY twice, so a typed `/` arrived as `//`.
 *
 *  Time cannot be the bound, because the duplicate is delivered relative to the next keystroke and not to a
 *  clock. Entries are therefore removed only by being matched, and this cap exists solely so a terminal where
 *  duplicates never arrive cannot grow the table without limit; it is far above any realistic typing burst and
 *  is reached only if the workaround stops matching at all. */
const DEDUP_MAX = 256;
/** Window for recognizing a reordered late keydown. An IME keydown (229) arriving this soon after an immediate
 *  input commit belongs to the same keystroke and must not arm keyup. Reordered events are only milliseconds apart,
 *  so 50 ms is generous while remaining much shorter than consecutive human keystrokes. */
const REORDERED_KEYDOWN_MS = 50;

export interface WebkitImeFix {
  /** Called from `term.onData`; returns true when data is xterm's late duplicate of a workaround commit. */
  shouldSwallow: (data: string) => boolean;
  dispose: () => void;
}

/**
 * Installs the WebKit IME punctuation workaround.
 * @param term xterm instance, already opened so `term.textarea` exists.
 * @param send Function that immediately writes to the PTY, wrapping `ptyWrite(sessionId, data)`.
 */
export function installWebkitImeFix(
  term: Terminal,
  send: (data: string) => void,
): WebkitImeFix {
  const textarea = term.textarea;
  // Workaround commits awaiting their late duplicate, oldest first. An entry is removed by being matched
  // (`shouldSwallow`); see DEDUP_MAX for why nothing else removes it. Entries store the half-width character the
  // commit stands for, so a duplicate is recognized whichever form of the same keystroke arrives.
  const pending: string[] = [];

  // Commit immediately to the PTY and register the text so xterm's later duplicate is consumed.
  const commitNow = (data: string) => {
    send(data);
    pending.push(halfwidthAscii(data) || data);
    // Safety valve only; a table at this size means duplicates stopped arriving entirely. See DEDUP_MAX.
    if (pending.length > DEDUP_MAX) pending.shift();
  };

  // State for keyup handling of half-width pass-through symbols. imeKeydown records the latest IME-processed
  // keydown (keyCode 229). English mode uses real codes and never enters this path. A timely input such as
  // full-width punctuation clears the marker before keyup; a delayed pass-through input leaves it armed so keyup
  // sends immediately.
  let imeKeydown: { code: string; at: number } | null = null;
  // Time of the latest immediate input commit, used to recognize a reordered late keydown.
  let lastInputCommitAt = 0;

  // WebKit emits only input for isolated full-width punctuation, so commit it immediately here.
  const onInput = (e: Event) => {
    const ie = e as InputEvent;
    // The parent listener sees input from the whole subtree; accept only this terminal's textarea.
    if (ie.target !== textarea) return;

    // A timely input is the IME commit, so clear imeKeydown to prevent keyup from sending again. A delayed
    // pass-through input arrives after keyup and merely clears an already empty marker.
    if (ie.inputType === "insertText") imeKeydown = null;

    if (ie.isComposing) return; // Leave active composition to xterm.
    if (ie.inputType !== "insertText") return; // Accept direct insertion only, excluding composition/deletion.
    const data = ie.data;
    if (!data) return; // Ignore empty commits.
    if (!isFullwidthPunct(data)) return; // Never intercept ASCII, Han text, digits, letters, kana, or Hangul.
    lastInputCommitAt = Date.now();
    commitNow(data);
  };

  // Record IME-processed keydown (229), but not real-code English input. If it arrives just after an immediate
  // input commit, it is WebKit's reordered keydown for the same keystroke; do not arm keyup or the half-width
  // source character would be sent in addition to the full-width punctuation.
  const onKeyDown = (e: Event) => {
    const ke = e as KeyboardEvent;
    if (ke.target !== textarea) return;
    if (ke.keyCode !== 229) return;
    if (Date.now() - lastInputCommitAt < REORDERED_KEYDOWN_MS) return;
    imeKeydown = { code: ke.code, at: Date.now() };
  };

  // Timely keyup contains the committed character. If it matches the recorded IME key and is a half-width ASCII
  // symbol, send immediately. Full-width punctuation is rejected by isPassthroughSymbol and stays on the input
  // path; only true pass-through symbols reach this branch.
  const onKeyUp = (e: Event) => {
    const ke = e as KeyboardEvent;
    if (ke.target !== textarea) return;
    if (ke.isComposing) return; // Leave pinyin/kana composition to the IME.
    if (
      !imeKeydown ||
      ke.code !== imeKeydown.code || // Must be the same physical key.
      Date.now() - imeKeydown.at > 1000 // Must be recent, excluding stale state.
    )
      return;
    if (!isPassthroughSymbol(ke.key)) return; // Accept only half-width ASCII symbols.
    imeKeydown = null; // Consume the marker to prevent repeated handling.
    commitNow(ke.key); // Send now; shouldSwallow consumes xterm's late duplicate.
  };

  // Attach all three listeners to the textarea's parent in the capture phase. Parent capture runs before xterm's
  // textarea listener, ensuring the deduplication entry exists whether xterm emits synchronously or later.
  const inputCaptureHost = textarea?.parentElement ?? textarea;
  inputCaptureHost?.addEventListener("input", onInput, true);
  inputCaptureHost?.addEventListener("keydown", onKeyDown, true);
  inputCaptureHost?.addEventListener("keyup", onKeyUp, true);

  return {
    shouldSwallow: (data: string) => {
      if (!pending.length) return false;
      // Compare the half-width form so the duplicate is recognized whichever channel committed first: a `keyup`
      // writing `/` and an input carrying the IME's `／` are the same keystroke, and only one belongs in the PTY.
      const key = halfwidthAscii(data) || data;
      const idx = pending.indexOf(key);
      if (idx === -1) return false;
      pending.splice(idx, 1);
      return true;
    },
    dispose: () => {
      inputCaptureHost?.removeEventListener("input", onInput, true);
      inputCaptureHost?.removeEventListener("keydown", onKeyDown, true);
      inputCaptureHost?.removeEventListener("keyup", onKeyUp, true);
      pending.length = 0;
    },
  };
}
