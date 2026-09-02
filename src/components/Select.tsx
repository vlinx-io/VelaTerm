//! The single dropdown used by every plain single-choice control in the app: a trigger button plus a
//! popup list of options. Before this existed the same button-and-popup markup had been copied into a
//! dozen files, and the copies had drifted apart in height, font size, panel background, shadow, and
//! hover colour, so two dropdowns sitting in the same dialog no longer looked alike.
//!
//! A native `<select>` is not used because WKWebView renders one as a light beveled system control
//! that ignores the theme, which is why the hand-rolled version appeared in the first place.
//!
//! Scope: one value, chosen from a fixed list, committed on click. Controls that also accept free
//! text (the model field) or that carry submenus (right-click menus, which belong to ContextMenu)
//! are not this component's job.

import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
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

export default function Select<T extends string>({
  value,
  onChange,
  options,
  width = 160,
  menuWidth,
  size = "md",
  mono = false,
  align = "left",
  placeholder,
  disabled,
  title,
  ariaLabel,
  leading,
}: {
  value: T;
  onChange: (v: T) => void;
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
}) {
  const s = SIZES[size];
  const [open, setOpen] = useState(false);
  // Keyboard focus is tracked separately from the committed value: arrow keys move the highlight and
  // only Enter commits, which is how a native select behaves.
  const [active, setActive] = useState(-1);
  const listRef = useRef<HTMLDivElement | null>(null);
  const btnRef = useRef<HTMLButtonElement | null>(null);
  const listId = useId();

  const selected = options.findIndex((o) => o.value === value);
  const current = selected >= 0 ? options[selected] : undefined;

  // Opening starts the highlight on the current value so Enter alone is a no-op rather than a jump.
  useEffect(() => {
    if (open) setActive(selected);
  }, [open, selected]);

  // Keep the highlighted row in view while arrowing through a list taller than the panel.
  useLayoutEffect(() => {
    if (!open || active < 0) return;
    const row = listRef.current?.children[active] as HTMLElement | undefined;
    // Guarded because jsdom, used by the tests, does not implement scrollIntoView.
    row?.scrollIntoView?.({ block: "nearest" });
  }, [open, active]);

  const commit = (i: number) => {
    const opt = options[i];
    if (!opt || opt.disabled) return;
    onChange(opt.value);
    setOpen(false);
    btnRef.current?.focus();
  };

  // Skips disabled rows so holding an arrow key never parks the highlight on an unselectable option.
  const step = (from: number, dir: 1 | -1) => {
    for (let i = from + dir; i >= 0 && i < options.length; i += dir) {
      if (!options[i].disabled) return i;
    }
    return from;
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      if (!open) return;
      e.preventDefault();
      setOpen(false);
      return;
    }
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!open) {
        setOpen(true);
        return;
      }
      const from = active < 0 ? (e.key === "ArrowDown" ? -1 : options.length) : active;
      setActive(step(from, e.key === "ArrowDown" ? 1 : -1));
      return;
    }
    if (e.key === "Home" || e.key === "End") {
      if (!open) return;
      e.preventDefault();
      setActive(e.key === "Home" ? step(-1, 1) : step(options.length, -1));
      return;
    }
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      if (!open) setOpen(true);
      else if (active >= 0) commit(active);
    }
  };

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
        {current?.icon}
        <span
          style={{
            flex: 1,
            textAlign: "left",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          {current?.label ?? placeholder ?? value}
        </span>
        <Icons.chevD size={12} style={{ color: "var(--text-muted)", flex: "none" }} />
      </button>

      {open && (
        <>
          {/* Transparent backdrop closes the popup on any outside click. mousedown rather than click so
              the popup is gone before the click lands on whatever is underneath. */}
          <div
            style={{ position: "fixed", inset: 0, zIndex: 1240 }}
            onMouseDown={() => setOpen(false)}
          />
          <div
            ref={listRef}
            id={listId}
            role="listbox"
            aria-label={ariaLabel}
            // A wrapping <label> would otherwise forward a row click on to the trigger button as a
            // second activation, reopening the popup the moment a choice closed it.
            onClick={(e) => e.stopPropagation()}
            style={{
              ...SELECT_PANEL,
              left: align === "left" ? 0 : undefined,
              right: align === "right" ? 0 : undefined,
              width: menuWidth ?? "100%",
              minWidth: "100%",
            }}
          >
            {options.map((opt, i) => {
              const on = i === selected;
              const hot = i === active;
              return (
                <div
                  key={opt.value || `_${i}`}
                  role="option"
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
        </>
      )}
    </div>
  );
}
