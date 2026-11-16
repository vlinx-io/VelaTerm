//! Editable combo box: the shared dropdown plus a text field, for choices that are usually picked from
//! a list but occasionally typed.
//!
//! A plain `Select` cannot serve this: its trigger is a button with no caret. A native `<datalist>` cannot
//! either, because WKWebView offers no list UI for it and the styling would be the browser's own. The
//! panel and row look of `Select` is reused here so a combo and a dropdown in the same dialog are one
//! family, and `useMenuPosition` keeps their popups anchored by the same rules.

import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import Icons from "./Icons";
import { SELECT_PANEL, selectRowStyle, useMenuPosition, type SelectOption } from "./Select";

/** Trigger metrics matching the `md` Select, which is what dialogs use. */
const HEIGHT = 32;
const FONT_SIZE = 12.5;

function labelOf<T extends string>(value: T, options: SelectOption<T>[]): string {
  return options.find((option) => option.value === value)?.label ?? value;
}

export default function Combo<T extends string>({
  value,
  onChange,
  onCommit,
  options,
  allowCustom = false,
  width = 160,
  menuWidth,
  align = "left",
  placeholder,
  disabled,
  title,
  ariaLabel,
  mono = false,
  menuPortal = false,
}: {
  value: T;
  /** Every committed value, and every keystroke while `allowCustom` is on. */
  onChange: (v: T) => void;
  /** A committed value only, so a caller can remember a choice without remembering a half-typed prefix. */
  onCommit?: (v: T) => void;
  options: SelectOption<T>[];
  /** Whether text matching no option is a value the caller can use (a model, an effort level). */
  allowCustom?: boolean;
  /** Trigger width. Pass "100%" to fill a flex row. */
  width?: number | string;
  /** Popup width when it should not track the trigger. */
  menuWidth?: number | string;
  /** Which edge the popup lines up with. */
  align?: "left" | "right";
  placeholder?: string;
  disabled?: boolean;
  title?: string;
  ariaLabel?: string;
  /** Monospace text, for values that are identifiers rather than prose. */
  mono?: boolean;
  /** Render the list outside clipping/scrolling parents, anchored to the field. */
  menuPortal?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(-1);
  // Whether the text in the field is a search phrase rather than the committed value. Tracked separately
  // from the text because a caller that accepts custom values mirrors every keystroke into `value`, so
  // comparing the text with the value cannot tell typing from a committed choice.
  const [filtering, setFiltering] = useState(false);
  const [text, setText] = useState(() => (value === "" ? "" : labelOf(value, options)));
  const boxRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const listRef = useRef<HTMLDivElement | null>(null);
  const listId = useId();
  const menuPosition = useMenuPosition(boxRef, listRef, open, menuPortal, menuWidth, align);

  // An empty value means "not set" and shows the placeholder; anything else shows its option label.
  const display = value === "" ? "" : labelOf(value, options);
  // An external value change — an agent switch clearing a model, say — replaces the draft text. While the
  // list is open the field belongs to the person typing, so it is left alone.
  useEffect(() => {
    if (!open) setText(display);
  }, [display, open]);

  // The list narrows only while the field is being searched. Opening it shows everything, so a committed
  // value that happens to share no letters with the options is not hidden behind its own text.
  const needle = filtering && open ? text.trim().toLowerCase() : "";
  const shown = needle
    ? options.filter(
        (option) =>
          option.label.toLowerCase().includes(needle) ||
          option.value.toLowerCase().includes(needle) ||
          (option.hint ?? "").toLowerCase().includes(needle),
      )
    : options;

  // An open field with nothing to show renders no panel at all: an empty slab reads as a broken menu.
  const expanded = open && shown.length > 0;

  const commit = (next: T, label: string) => {
    setOpen(false);
    setFiltering(false);
    setText(label);
    if (next !== value) onChange(next);
    onCommit?.(next);
  };

  // What Enter, Tab and blur settle on: the highlighted row if there is one, a row matching the text, or
  // the text itself when the caller accepts custom values. Anything else falls back to the committed value.
  const commitText = () => {
    const raw = text.trim();
    const match = options.find(
      (option) => option.value === raw || option.label.toLowerCase() === raw.toLowerCase(),
    );
    if (match) {
      commit(match.value, match.label);
      return;
    }
    if (allowCustom) {
      commit(raw as T, raw);
      return;
    }
    setText(display);
    setFiltering(false);
    setOpen(false);
  };

  // Skips disabled rows so holding an arrow key never parks the highlight on an unselectable option.
  const step = (from: number, dir: 1 | -1) => {
    for (let i = from + dir; i >= 0 && i < shown.length; i += dir) {
      if (!shown[i].disabled) return i;
    }
    return from;
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      if (!open) return;
      e.preventDefault();
      setText(display);
      setFiltering(false);
      setOpen(false);
      return;
    }
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!open) {
        setOpen(true);
        return;
      }
      const from = active < 0 ? (e.key === "ArrowDown" ? -1 : shown.length) : active;
      setActive(step(from, e.key === "ArrowDown" ? 1 : -1));
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      if (active >= 0 && shown[active]) commit(shown[active].value, shown[active].label);
      else commitText();
      return;
    }
    if (e.key === "Tab" && (open || allowCustom)) commitText();
  };

  // Keep the highlighted row in view while arrowing through a list taller than the panel.
  useLayoutEffect(() => {
    if (!open || active < 0) return;
    const row = listRef.current?.children[active] as HTMLElement | undefined;
    const list = listRef.current;
    if (!row || !list) return;
    if (row.offsetTop < list.scrollTop) list.scrollTop = row.offsetTop;
    else if (row.offsetTop + row.offsetHeight > list.scrollTop + list.clientHeight) {
      list.scrollTop = row.offsetTop + row.offsetHeight - list.clientHeight;
    }
  }, [open, active]);

  const menu = (
    <>
      {/* Transparent backdrop closes the popup on any outside click. mousedown rather than click so the
          popup is gone before the click lands on whatever is underneath. */}
      {menuPortal && (
        <div
          style={{ position: "fixed", inset: 0, zIndex: 1240 }}
          onMouseDown={() => setOpen(false)}
          // Portaled nodes still bubble through the React tree; without this the wrapper's label would
          // take the click and reopen the menu the moment it closed.
          onClick={(e) => e.stopPropagation()}
        />
      )}
      <div
        ref={listRef}
        id={listId}
        role="listbox"
        aria-label={ariaLabel}
        // A wrapping <label> would otherwise forward a row click on to the input as a second activation.
        // Label activation is a default action, so only preventDefault cancels it.
        onClick={(e) => { e.preventDefault(); e.stopPropagation(); }}
        style={{
          ...SELECT_PANEL,
          left: align === "left" ? 0 : undefined,
          right: align === "right" ? 0 : undefined,
          width: menuWidth ?? "100%",
          minWidth: "100%",
          ...(menuPortal ? menuPosition : {}),
        }}
      >
        {shown.map((option, i) => {
          const selected = option.value === value;
          const hot = i === active && !option.disabled;
          return (
            <div
              key={option.value || `_${i}`}
              role="option"
              aria-selected={selected}
              aria-disabled={option.disabled || undefined}
              // preventDefault keeps focus in the text field, so committing a row does not first blur the
              // field and settle on the typed text.
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => !option.disabled && commit(option.value, option.label)}
              onMouseEnter={() => !option.disabled && setActive(i)}
              style={{
                ...selectRowStyle(selected, hot, "md", mono),
                cursor: option.disabled ? "default" : "pointer",
                opacity: option.disabled ? 0.45 : 1,
              }}
            >
              {option.icon}
              <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                {option.label}
              </span>
              {option.hint && (
                <span style={{ flex: "none", color: "var(--text-muted)", fontSize: FONT_SIZE - 1 }}>
                  {option.hint}
                </span>
              )}
            </div>
          );
        })}
      </div>
    </>
  );

  return (
    <div
      ref={boxRef}
      style={{
        position: "relative",
        width,
        flex: width === "100%" ? 1 : undefined,
        minWidth: 0,
        display: "flex",
        alignItems: "center",
        height: HEIGHT,
        // A full-width combo grows along a row; in a column container its zero flex basis would otherwise
        // collapse the box to the text line height.
        minHeight: HEIGHT,
        background: "var(--bg-app)",
        border: `1px solid ${open ? "var(--accent)" : "var(--border)"}`,
        borderRadius: 6,
        opacity: disabled ? 0.6 : 1,
      }}
    >
      <input
        ref={inputRef}
        role="combobox"
        aria-haspopup="listbox"
        aria-expanded={expanded}
        aria-controls={expanded ? listId : undefined}
        aria-label={ariaLabel}
        title={title}
        disabled={disabled}
        value={text}
        placeholder={placeholder}
        spellCheck={false}
        autoComplete="off"
        onClick={() => {
          if (!disabled) setOpen(true);
        }}
        onChange={(e) => {
          const next = e.target.value;
          setText(next);
          setOpen(true);
          setFiltering(true);
          setActive(-1);
          if (allowCustom) onChange(next as T);
        }}
        onKeyDown={onKeyDown}
        onFocus={() => {
          setFiltering(false);
          setOpen(true);
          // Clicks land before some browsers finish moving the caret; selecting the text makes the first
          // keystroke replace the value rather than splice into it.
          inputRef.current?.select();
        }}
        onBlur={(e) => {
          if (!boxRef.current?.contains(e.relatedTarget as Node | null)) commitText();
        }}
        style={{
          flex: 1,
          minWidth: 0,
          height: "100%",
          padding: "0 4px 0 9px",
          background: "transparent",
          border: "none",
          outline: "none",
          color: "var(--text-primary)",
          fontFamily: mono ? "var(--font-mono)" : "inherit",
          fontSize: FONT_SIZE,
        }}
      />
      {options.length > 0 && (
        <button
          type="button"
          tabIndex={-1}
          aria-hidden="true"
          style={{
            display: "grid",
            placeItems: "center",
            flex: "none",
            width: 26,
            height: "100%",
            padding: 0,
            border: "none",
            background: "transparent",
            color: "var(--text-muted)",
            cursor: "pointer",
          }}
          onClick={() => {
            if (disabled) return;
            if (open) setOpen(false);
            else {
              inputRef.current?.focus();
              setOpen(true);
            }
          }}
        >
          <Icons.chevD size={12} />
        </button>
      )}
      {expanded && (menuPortal ? createPortal(menu, document.body) : menu)}
    </div>
  );
}
