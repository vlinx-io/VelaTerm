//! The shared dropdown for fixed single-choice and multiple-choice controls in the app: a trigger button plus a
//! popup list of options. Before this existed the same button-and-popup markup had been copied into a
//! dozen files, and the copies had drifted apart in height, font size, panel background, shadow, and
//! hover colour, so two dropdowns sitting in the same dialog no longer looked alike.
//!
//! A native `<select>` is not used because WKWebView renders one as a light beveled system control
//! that ignores the theme, which is why the hand-rolled version appeared in the first place.
//!
//! Scope: values chosen from a fixed list, committed on click. Multi-select keeps the menu open.
//! Long lists can opt into a filter box at the top of the popup (`searchable`); typing narrows the rows
//! by label, and the arrow keys and Enter keep working from inside the box.
//! Controls that also accept free text (the model field) or that carry submenus
//! (right-click menus, which belong to ContextMenu)
//! are not this component's job.

import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import Icons from "./Icons";

export type SelectOption<T extends string> = {
  value: T;
  label: string;
  /** Optional leading glyph, e.g. the agent icons in the agents panel. */
  icon?: React.ReactNode;
  /** Dimmed trailing text, e.g. a shell's path next to its name. */
  hint?: string;
  disabled?: boolean;
  /** Draws a rule above this row, for lists that fall into groups (built-in kinds vs saved presets). */
  separatorBefore?: boolean;
};

/** Trigger metrics. `sm` suits dense settings rows, `md` suits dialog form fields. */
const SIZES = {
  sm: { height: 26, fontSize: 11.5, padding: "0 8px", rowPadding: "5px 8px" },
  md: { height: 32, fontSize: 12.5, padding: "0 9px", rowPadding: "6px 8px" },
} as const;

export type SelectSize = keyof typeof SIZES;

/** Popup panel look, exported so the model combo box (an editable field with the same list hanging
 * off it) stays visually identical without duplicating the values. */
export const SELECT_PANEL: React.CSSProperties = {
  position: "absolute",
  top: "calc(100% + 4px)",
  zIndex: 1250,
  maxHeight: 260,
  overflowY: "auto",
  padding: 4,
  background: "var(--bg-elevated)",
  border: "1px solid var(--border-strong)",
  borderRadius: 8,
  boxShadow: "var(--shadow)",
};

/** One row of that list. `selected` is the committed value, `active` the keyboard or hover highlight. */
export function selectRowStyle(
  selected: boolean,
  active: boolean,
  size: SelectSize = "md",
  mono = false,
): React.CSSProperties {
  return {
    display: "flex",
    alignItems: "center",
    gap: 6,
    padding: SIZES[size].rowPadding,
    borderRadius: 6,
    cursor: "pointer",
    fontFamily: mono ? "var(--font-mono)" : "inherit",
    fontSize: SIZES[size].fontSize,
    color: selected ? "var(--accent)" : "var(--text-primary)",
    background: selected ? "var(--accent-soft)" : active ? "var(--bg-hover)" : "transparent",
  };
}

/** Anchors a portaled popup to its trigger and keeps it inside the viewport, flipping above when the
 * space below is tighter. Shared by `Select` and `Combo`, whose panels must sit in the same place. */
export function useMenuPosition(
  anchorRef: React.RefObject<HTMLElement | null>,
  listRef: React.RefObject<HTMLElement | null>,
  open: boolean,
  enabled: boolean,
  menuWidth?: number | string,
  align: "left" | "right" = "left",
): React.CSSProperties {
  const [menuPosition, setMenuPosition] = useState<React.CSSProperties>({});
  useLayoutEffect(() => {
    if (!open || !enabled) return;
    const position = () => {
      const rect = anchorRef.current?.getBoundingClientRect();
      if (!rect) return;
      const popupWidth = Math.min(typeof menuWidth === "number" ? menuWidth : rect.width, window.innerWidth - 16);
      const below = window.innerHeight - rect.bottom - 8;
      const above = rect.top - 8;
      const upwards = below < 260 && above > below;
      setMenuPosition({
        position: "fixed",
        right: undefined,
        left: Math.max(8, Math.min(align === "right" ? rect.right - popupWidth : rect.left, window.innerWidth - popupWidth - 8)),
        top: upwards ? undefined : rect.bottom + 4,
        bottom: upwards ? window.innerHeight - rect.top + 4 : undefined,
        width: popupWidth,
        maxHeight: Math.max(0, Math.min(260, (upwards ? above : below) - 4)),
        minWidth: 0,
        overscrollBehavior: "contain",
      });
    };
    const onScroll = (event: Event) => {
      if (event.target !== listRef.current) position();
    };
    position();
    window.addEventListener("resize", position);
    window.addEventListener("scroll", onScroll, true);
    return () => {
      window.removeEventListener("resize", position);
      window.removeEventListener("scroll", onScroll, true);
    };
  }, [anchorRef, listRef, open, enabled, menuWidth, align]);
  return menuPosition;
}

type SelectProps<T extends string> = ({
  multiple?: false;
  value: T;
  onChange: (value: T) => void;
} | {
  multiple: true;
  value: T[];
  onChange: (value: T[]) => void;
  selectAllLabel?: string;
  clearLabel?: string;
}) & {
  /** Optional summary for the selected values in the collapsed trigger. */
  summary?: string;
  options: SelectOption<T>[];
  /** Trigger width. Pass "100%" to fill a flex row. */
  width?: number | string;
  /** Popup width when it should not track the trigger, e.g. long labels under a narrow button. */
  menuWidth?: number | string;
  size?: SelectSize;
  /** Monospace trigger and rows, for values that are identifiers rather than prose. */
  mono?: boolean;
  /** Which edge the popup lines up with. */
  align?: "left" | "right";
  /** Shown dimmed when no option matches the current value. */
  placeholder?: string;
  disabled?: boolean;
  title?: string;
  ariaLabel?: string;
  /** Fixed caption sharing the trigger's box, divided by a rule (the remote panel's "Address" field). */
  leading?: string;
  /** Render long lists outside clipping/scrolling parents, anchored to the trigger. */
  menuPortal?: boolean;
  /** Commands listed under the options, divided by a rule (the New Session project list's "Import Project").
   * They are reached with the arrow keys like options; choosing one closes the popup and runs it. */
  actions?: SelectAction[];
  /** Puts a filter box at the top of the popup. Typing on the closed trigger opens it with that text. */
  searchable?: boolean;
  searchPlaceholder?: string;
  /** Shown in place of the rows when the filter matches nothing. */
  noMatchLabel?: string;
};

export type SelectAction = { id: string; label: string; icon?: React.ReactNode; onSelect: () => void };

export default function Select<T extends string>(props: SelectProps<T>) {
  const { value, options, width = 160, menuWidth, size = "md", mono = false, align = "left",
    placeholder, disabled, title, ariaLabel, leading, menuPortal = false, summary, actions = [],
    searchable = false, searchPlaceholder, noMatchLabel } = props;
  const multiple = props.multiple === true;
  const [query, setQuery] = useState("");
  const needle = searchable ? query.trim().toLocaleLowerCase() : "";
  // Rows on screen. Keyboard indices refer to this list, not to `options`.
  const shown = needle ? options.filter(o => o.label.toLocaleLowerCase().includes(needle)) : options;
  // Action rows follow the options in keyboard order, so an index past the options names an action.
  const total = shown.length + actions.length;

  const s = SIZES[size];
  const [open, setOpen] = useState(false);
  // Keyboard focus is tracked separately from the committed value: arrow keys move the highlight and
  // only Enter commits, which is how a native select behaves.
  const [active, setActive] = useState(-1);
  const listRef = useRef<HTMLDivElement | null>(null);
  // With actions the options scroll in their own box so the actions stay pinned under them.
  const optionsRef = useRef<HTMLDivElement | null>(null);
  const btnRef = useRef<HTMLButtonElement | null>(null);
  const searchRef = useRef<HTMLInputElement | null>(null);
  const listId = useId();
  const menuPosition = useMenuPosition(btnRef, listRef, open, menuPortal, menuWidth, align);

  const isOn = (o: SelectOption<T>) => props.multiple ? props.value.includes(o.value) : o.value === props.value;
  const current = props.multiple ? undefined : options.find(isOn);
  const selected = shown.findIndex(isOn);

  // Selection changes must not move the cursor while toggling several options.
  const wasOpen = useRef(false);
  useEffect(() => {
    if (open && (!wasOpen.current || !multiple)) setActive(selected);
    wasOpen.current = open;
  }, [open, selected, multiple]);
  // A new filter highlights its first match, so Enter picks what the user sees at the top.
  const lastNeedle = useRef(needle);
  useEffect(() => {
    if (lastNeedle.current === needle) return;
    lastNeedle.current = needle;
    if (needle) setActive(shown.findIndex(o => !o.disabled));
    else setActive(selected);
  }, [needle, shown, selected]);
  useEffect(() => {
    if (!open) setQuery("");
    else if (searchable) searchRef.current?.focus();
  }, [open, searchable]);
  useEffect(() => { if (disabled) setOpen(false); }, [disabled]);

  // Keep the highlighted row in view while arrowing through a list taller than the panel.
  useLayoutEffect(() => {
    if (!open || active < 0) return;
    const row = listRef.current?.querySelector<HTMLElement>(`[data-select-index="${active}"]`);
    const list = actions.length || searchable ? optionsRef.current : listRef.current;
    if (!row || !list) return;
    // Scroll the list only. scrollIntoView also moves surrounding settings panes under the pointer.
    if (row.offsetTop < list.scrollTop) list.scrollTop = row.offsetTop;
    else if (row.offsetTop + row.offsetHeight > list.scrollTop + list.clientHeight) {
      list.scrollTop = row.offsetTop + row.offsetHeight - list.clientHeight;
    }
  }, [open, active, actions.length, searchable]);

  const commit = (i: number) => {
    const action = i >= shown.length ? actions[i - shown.length] : undefined;
    if (action) {
      if (disabled) return;
      setOpen(false);
      action.onSelect();
      return;
    }
    const opt = shown[i];
    if (!opt || opt.disabled) return;
    if (disabled) return;
    if (props.multiple) {
      props.onChange(props.value.includes(opt.value) ? props.value.filter(value => value !== opt.value) : [...props.value, opt.value]);
    } else {
      props.onChange(opt.value);
      setOpen(false);
    }
    btnRef.current?.focus();
  };

  // Skips disabled rows so holding an arrow key never parks the highlight on an unselectable option.
  const step = (from: number, dir: 1 | -1) => {
    for (let i = from + dir; i >= 0 && i < total; i += dir) {
      if (!shown[i]?.disabled) return i;
    }
    return from;
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    // Inside the filter box, Cmd/Ctrl+A, Home and End edit the text instead of the selection.
    if (open && props.multiple && !searchable && (e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a") {
      e.preventDefault();
      const all = options.filter(o => !o.disabled).every(o => props.value.includes(o.value));
      props.onChange(options.filter(o => o.disabled ? props.value.includes(o.value) : !all).map(o => o.value));
      return;
    }
    if (e.key === "Escape") {
      if (!open) return;
      e.preventDefault();
      e.stopPropagation();
      setOpen(false);
      btnRef.current?.focus();
      return;
    }
    if (e.key === "Tab") {
      if (open && searchable) { e.preventDefault(); btnRef.current?.focus(); }
      setOpen(false);
      return;
    }
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!open) {
        setOpen(true);
        return;
      }
      const from = active < 0 ? (e.key === "ArrowDown" ? -1 : total) : active;
      setActive(step(from, e.key === "ArrowDown" ? 1 : -1));
      return;
    }
    if (e.key === "Home" || e.key === "End") {
      if (!open || searchable) return;
      e.preventDefault();
      setActive(e.key === "Home" ? step(-1, 1) : step(total, -1));
      return;
    }
    if (e.key === "Enter" || (e.key === " " && !(searchable && open))) {
      e.preventDefault();
      if (!open) setOpen(true);
      else if (active >= 0) commit(active);
      return;
    }
    // A printable key on the closed trigger opens the filter already holding that character.
    if (searchable && !open && e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
      e.preventDefault();
      setQuery(e.key);
      setOpen(true);
    }
  };

  const renderMenu = (menu: React.ReactNode) => menuPortal ? createPortal(menu, document.body) : menu;

  return (
    <div
      style={{
        position: "relative",
        width,
        flex: width === "100%" ? 1 : undefined,
        minWidth: 0,
        // With a caption the border moves to this wrapper, so caption and button read as one control.
        display: leading ? "flex" : undefined,
        alignItems: leading ? "center" : undefined,
        boxSizing: leading ? "border-box" : undefined,
        background: leading ? "var(--bg-app)" : undefined,
        border: leading ? `1px solid ${open ? "var(--accent)" : "var(--border)"}` : undefined,
        borderRadius: leading ? 6 : undefined,
      }}
    >
      {leading && (
        <span
          style={{
            flex: "none",
            padding: s.padding,
            lineHeight: `${s.height - 2}px`,
            fontSize: s.fontSize,
            color: "var(--text-muted)",
            whiteSpace: "nowrap",
            borderRight: "1px solid var(--border)",
          }}
        >
          {leading}
        </span>
      )}
      <button
        ref={btnRef}
        type="button"
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-activedescendant={open && active >= 0 ? `${listId}-${active}` : undefined}
        aria-label={ariaLabel}
        title={title}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
        onKeyDown={onKeyDown}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          width: "100%",
          height: s.height,
          padding: s.padding,
          background: leading ? "transparent" : "var(--bg-app)",
          color: current ? "var(--text-primary)" : "var(--text-muted)",
          border: leading ? "none" : `1px solid ${open ? "var(--accent)" : "var(--border)"}`,
          borderRadius: leading ? 0 : 6,
          flex: leading ? 1 : undefined,
          minWidth: 0,
          outline: "none",
          fontFamily: mono ? "var(--font-mono)" : "inherit",
          fontSize: s.fontSize,
          cursor: disabled ? "default" : "pointer",
          opacity: disabled ? 0.6 : 1,
        }}
      >
        {!multiple && current?.icon}
        <span
          style={{
            flex: 1,
            textAlign: "left",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {summary ?? (multiple ? options.filter(o => (value as T[]).includes(o.value)).map(o => o.label).join(", ") || placeholder : current?.label ?? placeholder ?? value)}
        </span>
        <Icons.chevD size={12} style={{ color: "var(--text-muted)", flex: "none" }} />
      </button>

      {open && renderMenu(
        <>
          {/* Transparent backdrop closes the popup on any outside click. mousedown rather than click so
              the popup is gone before the click lands on whatever is underneath. */}
          <div
            style={{ position: "fixed", inset: 0, zIndex: 1240 }}
            onMouseDown={() => setOpen(false)}
          />
          <div
            ref={listRef}
            id={multiple ? undefined : listId}
            role={multiple ? undefined : "listbox"}
            aria-label={multiple ? undefined : ariaLabel}
            // The filter box needs the mousedown to take focus and place its caret.
            onMouseDown={e => { if (e.target !== searchRef.current) e.preventDefault(); }}
            // A wrapping <label> would otherwise forward a row click on to the trigger button as a
            // second activation, reopening the popup the moment a choice closed it. Label activation
            // is a default action, so only preventDefault cancels it; stopPropagation alone does not.
            onClick={(e) => { e.preventDefault(); e.stopPropagation(); }}
            style={{
              ...SELECT_PANEL,
              left: align === "left" ? 0 : undefined,
              right: align === "right" ? 0 : undefined,
              width: menuWidth ?? "100%",
              minWidth: "100%",
              ...(menuPortal ? menuPosition : {}),
              ...(actions.length || searchable ? { display: "flex", flexDirection: "column", overflowY: "hidden" } : {}),
            }}
          >
            {searchable && <div role="presentation" style={{ flex: "none", padding: "0 0 4px", marginBottom: 4, borderBottom: "1px solid var(--border)" }}>
              <input
                ref={searchRef}
                type="text"
                value={query}
                placeholder={searchPlaceholder}
                aria-label={searchPlaceholder ?? ariaLabel}
                aria-controls={listId}
                aria-activedescendant={active >= 0 ? `${listId}-${active}` : undefined}
                autoComplete="off"
                autoCorrect="off"
                autoCapitalize="off"
                spellCheck={false}
                onChange={e => setQuery(e.target.value)}
                onKeyDown={onKeyDown}
                style={{
                  display: "block",
                  width: "100%",
                  boxSizing: "border-box",
                  height: s.height - 4,
                  padding: s.padding,
                  background: "var(--bg-app)",
                  color: "var(--text-primary)",
                  border: "1px solid var(--border)",
                  borderRadius: 6,
                  outline: "none",
                  fontFamily: mono ? "var(--font-mono)" : "inherit",
                  fontSize: s.fontSize,
                }}
              />
            </div>}
            {props.multiple && (props.selectAllLabel || props.clearLabel) && <div role="presentation" style={{ display: "flex", gap: 8, padding: "4px 4px 8px", borderBottom: "1px solid var(--border)", marginBottom: 4 }}>
              {props.selectAllLabel && <button type="button" className="vlx-btn" onClick={() => props.onChange(options.filter(o => !o.disabled || props.value.includes(o.value)).map(o => o.value))}>{props.selectAllLabel}</button>}
              {props.clearLabel && <button type="button" className="vlx-btn" onClick={() => props.onChange(options.filter(o => o.disabled && props.value.includes(o.value)).map(o => o.value))}>{props.clearLabel}</button>}
            </div>}
            <div ref={optionsRef} id={multiple ? listId : undefined} role={multiple ? "listbox" : undefined} aria-label={multiple ? ariaLabel : undefined} aria-multiselectable={multiple || undefined}
              style={actions.length || searchable ? { position: "relative", flex: "1 1 auto", minHeight: 0, overflowY: "auto" } : undefined}>
            {needle && !shown.length && noMatchLabel && (
              <div role="presentation" style={{ padding: s.rowPadding, fontSize: s.fontSize, color: "var(--text-muted)" }}>{noMatchLabel}</div>
            )}
            {shown.map((opt, i) => {
              const on = isOn(opt);
              const hot = i === active;
              return (
                <div
                  key={opt.value || `_${i}`}
                  role="option"
                  id={`${listId}-${i}`}
                  data-select-index={i}
                  aria-selected={on}
                  aria-disabled={opt.disabled || undefined}
                  onClick={() => commit(i)}
                  onMouseEnter={() => !opt.disabled && setActive(i)}
                  style={{
                    ...selectRowStyle(on, hot && !opt.disabled, size, mono),
                    borderTop: opt.separatorBefore ? "1px solid var(--border)" : undefined,
                    marginTop: opt.separatorBefore ? 4 : undefined,
                    cursor: opt.disabled ? "default" : "pointer",
                    opacity: opt.disabled ? 0.45 : 1,
                  }}
                >
                  {multiple && <span aria-hidden="true" style={{ display: "inline-flex", alignItems: "center", justifyContent: "center", flex: "none", width: 16, height: 16, border: `1px solid ${on ? "var(--accent)" : "var(--border-strong)"}`, borderRadius: 3, background: on ? "var(--accent)" : "transparent", color: "var(--text-on-accent)", boxSizing: "border-box" }}>{on && <Icons.check size={12} />}</span>}
                  {opt.icon}
                  <span
                    style={{
                      flex: 1,
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {opt.label}
                  </span>
                  {opt.hint && (
                    <span
                      style={{
                        flex: "none",
                        color: "var(--text-muted)",
                        fontSize: s.fontSize - 1,
                      }}
                    >
                      {opt.hint}
                    </span>
                  )}
                </div>
              );
            })}
            </div>
            {actions.length > 0 && <div role="group" style={{ flex: "none", marginTop: 4, paddingTop: 4, borderTop: "1px solid var(--border)" }}>
              {actions.map((action, k) => {
                const i = shown.length + k;
                return (
                  <div
                    key={action.id}
                    role="option"
                    id={`${listId}-${i}`}
                    data-select-index={i}
                    aria-selected={false}
                    onClick={() => commit(i)}
                    onMouseEnter={() => setActive(i)}
                    style={{ ...selectRowStyle(false, i === active, size), color: "var(--text-secondary)" }}
                  >
                    {action.icon}
                    <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{action.label}</span>
                  </div>
                );
              })}
            </div>}
          </div>
        </>
      )}
    </div>
  );
}
