import type { Terminal } from "@xterm/xterm";
import { invoke } from "../../ipc/transport";
import { t } from "../../i18n";
import { getCompletionConfig, reloadCompletionConfig, watchCompletionConfig } from "./config";
import "./completion.css";

interface Item { index: number; label: string; description: string; matches: [number, number][] }
interface Snapshot {
  configured: boolean; supported: boolean; ready: boolean; revision: number; inputVersion: number;
  pending: boolean; query: string; items: Item[];
}
const delay = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms));
let nextMenuId = 0;

/** Shells own editing and matching; the popup only keeps the current presentation and selection. */
export function installCompletion(term: Terminal, write: (data: string) => Promise<unknown>, sessionId: string) {
  let disposed = false, composing = false, generation = 0, active = 0, requesting = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let snapshot: Snapshot | null = null, displayed: Snapshot | null = null;
  let queuedSelection: { generation: number; label: string } | null = null;
  let queuedQuery: boolean | null = null;
  // Set once the user moves the selection with the keyboard; Enter then accepts it like Tab.
  let searchMode = false, autoDue = false, picked = false;
  let canComplete = false, bootstrapping = true, checkingSupport = false;
  let above: boolean | null = null;
  let anchorColumn = 0;
  const menu = document.createElement("div");
  menu.className = "terminal-completion";
  menu.setAttribute("role", "listbox");
  menu.setAttribute("aria-label", t("settings.completionMode"));
  // Check mounted IDs as well, since hot reload can reset the module counter.
  do { menu.id = `terminal-completion-${++nextMenuId}`; } while (document.getElementById(menu.id));
  menu.hidden = true;
  const list = document.createElement("div"); list.className = "terminal-completion-list";
  const detail = document.createElement("div"); detail.className = "terminal-completion-detail";
  const footer = document.createElement("div"); footer.className = "terminal-completion-footer";
  const status = document.createElement("span");
  const keys = document.createElement("span"); keys.textContent = "↑ ↓  ·  Tab  ·  Esc";
  keys.setAttribute("aria-hidden", "true");
  footer.append(status, keys); menu.append(list, detail, footer); document.body.appendChild(menu);
  if (term.options?.fontFamily) menu.style.fontFamily = term.options.fontFamily;

  const call = (action: string, extra = {}) => invoke<Snapshot>("pty_completion", { sessionId, action, ...extra });
  function support(result: Snapshot) {
    canComplete = result.supported;
    bootstrapping = result.configured && !result.supported;
  }
  function refreshSupport() {
    if (checkingSupport || disposed) return;
    checkingSupport = true;
    void call("state").then(support).catch(() => { canComplete = false; })
      .finally(() => { checkingSupport = false; });
  }
  const focused = () => !disposed && !composing && document.activeElement === term.textarea
    && !!term.element?.getClientRects().length && term.buffer.active.type === "normal";
  function hide() {
    menu.hidden = true; snapshot = null; displayed = null; queuedSelection = null; above = null; picked = false;
    menu.removeAttribute("aria-busy");
    term.textarea?.removeAttribute("aria-activedescendant");
    term.textarea?.removeAttribute("aria-controls");
  }
  function cancel() { generation++; autoDue = false; queuedQuery = null; clearTimeout(timer); timer = undefined; hide(); }
  function position() {
    if (menu.hidden) return;
    const screen = term.element?.querySelector(".xterm-screen");
    if (!screen || !focused()) { cancel(); return; }
    const bounds = screen.getBoundingClientRect(), buffer = term.buffer.active;
    const row = buffer.baseY + buffer.cursorY - buffer.viewportY;
    if (row < 0 || row >= term.rows) { cancel(); return; }
    const cellWidth = bounds.width / term.cols, cellHeight = bounds.height / term.rows;
    // Anchor to the completion token, measuring rendered cells so CJK and surrogate pairs align.
    let token = displayed?.query ?? "";
    const selectedLabel = displayed?.items[active]?.label.replace(/[\\/]$/, "") ?? "";
    if (!/[\\/]/.test(selectedLabel)) token = token.split(/[\\/]/).pop() ?? token;
    let column = buffer.cursorX, remaining = token.length;
    const line = buffer.getLine?.(buffer.baseY + buffer.cursorY);
    while (column > 0 && remaining > 0) {
      column--;
      remaining -= line?.getCell(column)?.getChars().length ?? 1;
    }
    if (snapshot) anchorColumn = column;
    else column = anchorColumn;
    const x = bounds.left + cellWidth * column, y = bounds.top + cellHeight * row;
    const contentWidth = Math.max(180, ...[...list.children].map(row => {
      const label = row.querySelector<HTMLElement>(".terminal-completion-label");
      const description = row.querySelector<HTMLElement>(".terminal-completion-description");
      return 34 + (label?.scrollWidth ?? 0) + (description?.textContent ? 13 + Math.min(220, description.scrollWidth) : 0);
    }));
    menu.style.width = `${Math.min(contentWidth, 430, bounds.width || 430, window.innerWidth - 16)}px`;
    const top = Math.max(8, bounds.top), bottom = Math.min(window.innerHeight - 8, bounds.bottom);
    if (above === null) above = bottom - y - cellHeight < 240 && y - top > bottom - y - cellHeight;
    menu.style.maxHeight = `${Math.max(72, above ? y - top - 3 : bottom - y - cellHeight - 3)}px`;
    menu.style.left = `${Math.max(bounds.left, Math.min(x, bounds.right - menu.offsetWidth, window.innerWidth - menu.offsetWidth - 8))}px`;
    menu.style.top = `${above ? Math.max(top, y - menu.offsetHeight - 3) : y + cellHeight + 3}px`;
  }
  function highlight(scroll = true) {
    [...list.children].forEach((node, i) => node.setAttribute("aria-selected", String(i === active)));
    const selected = list.children[active] as HTMLElement | undefined;
    if (scroll) selected?.scrollIntoView({ block: "nearest" });
    if (selected) term.textarea?.setAttribute("aria-activedescendant", selected.id);
    const item = displayed?.items[active];
    detail.textContent = item?.description ?? ""; detail.hidden = !detail.textContent;
    status.textContent = snapshot ? `${active + 1} / ${displayed?.items.length ?? 0}` : t("common.loading");
    position();
  }
  async function accept(index: number) {
    if (!snapshot) {
      const item = displayed?.items[index];
      if (item) queuedSelection = { generation, label: item.label };
      clearTimeout(timer); autoDue = true;
      if (!requesting) void query(false);
      return;
    }
    const result = snapshot, item = result.items[index];
    cancel();
    if (!item) return;
    try {
      await call("accept", { revision: result.revision, index: item.index ?? index });
    } catch { /* The shell rejects changed input. */ }
    term.focus();
  }
  function show(result: Snapshot) {
    if (!result.items.length || !focused()) { hide(); return; }
    const queued = queuedSelection;
    const selectedLabel = displayed?.items[active]?.label;
    snapshot = result; displayed = result;
    active = Math.max(0, result.items.findIndex(item => item.label === selectedLabel));
    menu.removeAttribute("aria-busy"); list.replaceChildren();
    result.items.forEach((item, index) => {
      const row = document.createElement("div"); row.className = "terminal-completion-row";
      row.id = `${menu.id}-${index}`; row.setAttribute("role", "option");
      const icon = document.createElement("span"); icon.className = "terminal-completion-icon";
      const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
      svg.setAttribute("viewBox", "0 0 16 16"); svg.setAttribute("fill", "none");
      svg.setAttribute("stroke", "currentColor"); svg.setAttribute("stroke-width", "1.2");
      const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
      path.setAttribute("d", item.label.endsWith("/") ? "M2 4h4l2 2h6v7H2V4Z"
        : item.label.startsWith("-") ? "M3 5h10M3 11h10M6 3v4M10 9v4" : "M3 4l4 4-4 4m5 0h5");
      svg.append(path); icon.append(svg);
      icon.setAttribute("aria-hidden", "true");
      const label = document.createElement("span"); label.className = "terminal-completion-label";
      let offset = 0;
      for (const [start, end] of item.matches ?? []) {
        label.append(document.createTextNode(item.label.slice(offset, start)));
        const mark = document.createElement("mark"); mark.textContent = item.label.slice(start, end); label.append(mark); offset = end;
      }
      label.append(document.createTextNode(item.label.slice(offset)));
      const description = document.createElement("span"); description.className = "terminal-completion-description";
      description.textContent = item.description; row.title = item.description || item.label;
      row.append(icon, label, description);
      row.addEventListener("mousemove", () => { if (active !== index) { active = index; highlight(false); } });
      row.addEventListener("mousedown", e => { e.preventDefault(); void accept(index); });
      list.appendChild(row);
    });
    menu.hidden = false; term.textarea?.setAttribute("aria-controls", menu.id); highlight();
    if (queued?.generation === generation) {
      queuedSelection = null;
      const index = result.items.findIndex(item => item.label === queued.label);
      if (index >= 0) void accept(index);
    }
  }
  async function query(manual: boolean) {
    clearTimeout(timer); timer = undefined;
    if (!focused() || getCompletionConfig()?.available === false || getCompletionConfig()?.mode === "off") return;
    if (requesting) { queuedQuery = manual; return; }
    autoDue = false;
    const version = generation; requesting = true;
    try {
      let result = await call("query"); support(result);
      if (disposed) return;
      if (!result.supported || !result.ready) {
        if (version === generation) { hide(); if (manual) await write("\t"); }
        return;
      }
      const deadline = performance.now() + 2000;
      while (result.pending && performance.now() < deadline && !disposed) {
        await delay(16); result = await call("state");
      }
      if (disposed || version !== generation) return;
      if (result.pending) { hide(); return; }
      if (manual && result.items.length === 0) { hide(); await write("\t"); return; }
      show(result);
    } catch { if (version === generation) hide(); }
    finally {
      requesting = false;
      if (queuedQuery !== null && focused()) {
        const nextManual = queuedQuery; queuedQuery = null;
        clearTimeout(timer); timer = setTimeout(() => void query(nextManual), 0);
      } else if (autoDue && focused() && getCompletionConfig()?.mode === "auto") {
        clearTimeout(timer); timer = setTimeout(() => void query(false), 0);
      }
    }
  }
  function input(data: string) {
    generation++; autoDue = false; queuedSelection = null; queuedQuery = null; snapshot = null; picked = false;
    const paste = data.startsWith("\x1b[200~") && data.endsWith("\x1b[201~");
    if (/[\x12\x13]/.test(data) || data === "\x1b") searchMode = true;
    if (/[\r\n\x03]/.test(data)) searchMode = false;
    // Arrow keys walk the shell's own history. Requesting suggestions between two presses would run the
    // query widget in the middle of that walk, and Zsh's continuing history search ends after the first
    // entry once another widget runs, so Up would stop recalling anything. Leave the line to the shell;
    // typing again resumes automatic suggestions.
    if (/^\x1b(\[[AB]|O[AB])$/.test(data)) { cancel(); return; }
    if (!focused() || searchMode || getCompletionConfig()?.mode !== "auto" || (!paste && /[\r\n\x03\x04\x1a]/.test(data))) { cancel(); return; }
    // Keep the list mounted while native results update; never accept its stale revision.
    if (!menu.hidden && !/[\s\x00-\x1f]/.test(data)) {
      menu.setAttribute("aria-busy", "true"); status.textContent = t("common.loading");
    } else hide();
    autoDue = true;
    if (!requesting && timer === undefined) timer = setTimeout(() => void query(false), getCompletionConfig()!.debounceMs);
  }
  function key(event: KeyboardEvent): boolean {
    if (event.type !== "keydown" || event.isComposing || composing || !focused()) return true;
    if (getCompletionConfig()?.mode === "off") { cancel(); return true; }
    if (event.key === "Enter") {
      // An untouched list leaves Enter to the shell so typed commands still run as written.
      if (!menu.hidden && picked && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
        event.preventDefault(); void accept(active); return false;
      }
      cancel(); return true;
    }
    if (!menu.hidden && !event.ctrlKey && !event.altKey && !event.metaKey && !event.shiftKey) {
      if (["ArrowDown", "ArrowUp", "PageDown", "PageUp"].includes(event.key)) {
        const last = list.children.length - 1;
        const delta = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : event.key === "PageDown" ? 8 : -8;
        const next = Math.max(0, Math.min(last, active + delta));
        // A plain arrow that cannot move the selection belongs to the shell: closing the list here keeps
        // the previous command reachable with Up instead of swallowing the key on the first candidate.
        if (next === active && (event.key === "ArrowUp" || event.key === "ArrowDown")) { cancel(); return true; }
        active = next; picked = true;
        highlight(); event.preventDefault(); return false;
      }
      if (event.key === "Tab") { event.preventDefault(); void accept(active); return false; }
      if (event.key === "Escape") { cancel(); event.preventDefault(); return false; }
    }
    const trigger = (event.key === "Tab" && !event.ctrlKey) || (event.code === "Space" && event.ctrlKey);
    if (trigger && canComplete && !searchMode && !event.altKey && !event.metaKey && !event.shiftKey && getCompletionConfig()) {
      event.preventDefault(); cancel(); void query(event.key === "Tab"); return false;
    }
    return true;
  }
  const startComposition = () => { composing = true; cancel(); };
  const endComposition = () => { composing = false; };
  const focus = () => { refreshSupport(); void reloadCompletionConfig(); };
  const textarea = term.textarea;
  textarea?.addEventListener("compositionstart", startComposition); textarea?.addEventListener("compositionend", endComposition);
  textarea?.addEventListener("blur", cancel); textarea?.addEventListener("focus", focus);
  window.addEventListener("resize", position);
  const cursor = term.onCursorMove(position), scroll = term.onScroll(cancel);
  let checkingOutput = false;
  const output = term.onWriteParsed(() => {
    if (bootstrapping) refreshSupport();
    if (!snapshot || checkingOutput || requesting) return;
    const shown = snapshot; checkingOutput = true;
    void call("state").then(current => {
      if (snapshot === shown && (!current.ready || current.pending || !current.items.length
        || current.inputVersion !== shown.inputVersion || current.revision !== shown.revision)) cancel();
    }).catch(cancel).finally(() => { checkingOutput = false; });
  });
  const stopConfig = watchCompletionConfig(cancel); refreshSupport();
  return { input, key, dispose() {
    disposed = true; cancel(); stopConfig(); cursor.dispose(); scroll.dispose(); output.dispose(); menu.remove();
    textarea?.removeEventListener("compositionstart", startComposition); textarea?.removeEventListener("compositionend", endComposition);
    textarea?.removeEventListener("blur", cancel); textarea?.removeEventListener("focus", focus); window.removeEventListener("resize", position);
  } };
}
