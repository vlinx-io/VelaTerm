//! The chips on the composer's control row: model, thinking effort, collaboration style, permission mode.
//!
//! Each is a borderless 28px chip — a glyph, a label, and a caret — that opens its list upward, because
//! the row sits at the bottom of the pane and a downward menu would fall off the window. The popup reuses
//! the panel and row styling of the app's shared `Select`, so a menu opened here looks like every other
//! menu in the app even though the trigger deliberately does not.

import { useEffect, useRef, useState, type ReactNode } from "react";

import Icons from "../../../components/Icons";
import { SELECT_PANEL, selectRowStyle } from "../../../components/Select";

/** Lists longer than this get a filter box; shorter ones are read at a glance. */
const FILTER_THRESHOLD = 12;

export interface ChipOption<T extends string> {
  value: T;
  label: string;
  /** Dimmed trailing text, for a value that needs a word of explanation. */
  hint?: string;
  /** Mark drawn in the row's left margin, so a list can be scanned by shape before it is read. */
  glyph?: ReactNode;
}

/**
 * A level on a five-step scale, drawn as a short bar filled to that step over a faint track. The plainest
 * way to show that a list of five values is ordered, and where one of them sits, without inventing a
 * symbol for it.
 */
export function LevelBar({ level, of = 5 }: { level: number; of?: number }) {
  const track = 16;
  const filled = (track * Math.max(0, Math.min(level, of))) / of;
  return (
    <svg width={18} height={14} viewBox="0 0 20 16" aria-hidden="true">
      <rect
        x={2}
        y={6}
        width={track}
        height={4}
        rx={2}
        fill="currentColor"
        opacity={0.22}
      />
      {filled > 0 && (
        <rect
          x={2}
          y={6}
          width={filled}
          height={4}
          rx={2}
          fill="var(--accent)"
        />
      )}
    </svg>
  );
}

/**
 * One control on the composer row.
 *
 * `label` is what the chip reads now; passing an empty string leaves the glyph alone, which is how the row
 * stays usable in a narrow pane.
 */
export function ControlChip<T extends string>({
  glyph,
  label,
  title,
  value,
  options,
  onPick,
  disabled,
  menuWidth = 220,
  keepLabel,
  filterPlaceholder,
  defaultValue,
}: {
  glyph: ReactNode;
  label?: string;
  title: string;
  value: T;
  options: ChipOption<T>[];
  /** `keep` is whether the box below the list was ticked when this value was chosen. */
  onPick: (value: T, keep: boolean) => void;
  disabled?: boolean;
  menuWidth?: number;
  /**
   * Label for a box under the list that carries the choice beyond this conversation. Leaving it out
   * drops the box, which is how a control that has nothing to remember keeps its menu plain.
   */
  keepLabel?: string;
  /**
   * Placeholder of a filter box above the list, shown only once the list is long enough to need one. A
   * catalogue of a few hundred models cannot be scrolled through; it can be typed into.
   */
  filterPlaceholder?: string;
  /** Saved default, independent of the current conversation's selection. */
  defaultValue?: T;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  // Whether this choice should outlast the conversation. It starts unticked every time the menu opens:
  // changing what answers you right now is the ordinary act, and changing what every future conversation
  // starts with is the deliberate one.
  const [keep, setKeep] = useState(false);
  const [hover, setHover] = useState(false);
  const boxRef = useRef<HTMLDivElement | null>(null);

  // Every opening starts from the same place: the box is a decision about this one choice, not a setting
  // of its own that stays ticked.
  useEffect(() => {
    if (open) {
      setKeep(false);
      setQuery("");
    }
  }, [open]);

  const filtering = filterPlaceholder !== undefined && options.length > FILTER_THRESHOLD;
  const needle = query.trim().toLowerCase();
  const shown =
    filtering && needle
      ? options.filter(
          (option) =>
            option.label.toLowerCase().includes(needle) ||
            (option.hint ?? "").toLowerCase().includes(needle) ||
            option.value.toLowerCase().includes(needle),
        )
      : options;

  // Close on any click outside. mousedown rather than click, so the menu is gone before the click lands
  // on whatever is underneath it.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!boxRef.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div className="sv-chip-box" ref={boxRef}>
      <button
        className="sv-chip"
        title={title}
        disabled={disabled}
        style={open || hover ? { background: "var(--bg-hover)" } : undefined}
        onMouseEnter={() => setHover(true)}
        onMouseLeave={() => setHover(false)}
        onClick={() => setOpen((v) => !v)}
      >
        <span className="sv-chip-glyph">{glyph}</span>
        {label ? <span className="sv-chip-label">{label}</span> : null}
        <Icons.chevD size={12} />
      </button>
      {open && (
        // Anchored to the bottom edge: this row lives at the bottom of the pane.
        // The panel scrolls its list, not the box under it: a menu of a dozen models would otherwise hide
        // that box below the fold, where nobody would find it.
        <div
          style={{
            ...SELECT_PANEL,
            fontFamily: "var(--chat-font)",
            top: "auto",
            bottom: "calc(100% + 4px)",
            left: 0,
            width: menuWidth,
            maxHeight: "none",
            overflow: "visible",
            display: "flex",
            flexDirection: "column",
          }}
        >
          {filtering ? (
            <input
              className="sv-chip-filter"
              type="text"
              autoFocus
              value={query}
              placeholder={filterPlaceholder}
              onChange={(e) => setQuery(e.target.value)}
              // Typing into the box is not choosing a value, so the menu stays open under it.
              onMouseDown={(e) => e.stopPropagation()}
              onKeyDown={(e) => {
                if (e.key === "Enter" && shown.length > 0) {
                  e.preventDefault();
                  onPick(shown[0].value, keep);
                  setOpen(false);
                }
              }}
            />
          ) : null}
          <div style={{ maxHeight: 260, overflowY: "auto", minHeight: 0 }}>
            {shown.map((option) => (
              <div
                key={option.value}
                role="option"
                aria-selected={option.value === value}
                // A row that explains itself stacks: name, then the explanation under it. Side by side, a
                // long explanation squeezes the name until it wraps, and since each one is a different
                // length no two rows break in the same place — the list ends up looking shuffled.
                style={{
                  ...selectRowStyle(option.value === value, false, "sm"),
                  flexDirection: "column",
                  alignItems: "stretch",
                  gap: 1,
                }}
                onMouseDown={(e) => {
                  e.preventDefault();
                  onPick(option.value, keep);
                  setOpen(false);
                }}
              >
                <span style={{ display: "flex", alignItems: "center", gap: 7 }}>
                  {option.glyph ? (
                    <span
                      style={{
                        flex: "none",
                        display: "grid",
                        placeItems: "center",
                        width: 18,
                        color: "var(--text-dim)",
                        // A mark that carries its own colour keeps it; only the track and the like stay faint.
                      }}
                    >
                      {option.glyph}
                    </span>
                  ) : null}
                  {option.label}
                  {option.value === defaultValue && (
                    <span
                      style={{
                        marginLeft: "auto",
                        flexShrink: 0,
                        padding: "1px 5px",
                        border: "1px solid var(--border)",
                        borderRadius: 4,
                        color: "var(--text-dim)",
                        fontSize: 10,
                        lineHeight: 1.4,
                      }}
                    >
                      Default
                    </span>
                  )}
                </span>
                {option.hint ? (
                  <span
                    style={{
                      color: "var(--text-faint)",
                      fontSize: 11,
                      lineHeight: 1.4,
                      overflowWrap: "anywhere",
                      // Indent under the name rather than under the mark, so the explanation reads as
                      // belonging to the row's text.
                      paddingLeft: option.glyph ? 25 : 0,
                    }}
                  >
                    {option.hint}
                  </span>
                ) : null}
              </div>
            ))}
          </div>
          {keepLabel ? (
            <label
              style={{
                display: "flex",
                alignItems: "center",
                gap: 7,
                marginTop: 4,
                padding: "7px 8px",
                borderTop: "1px solid var(--border)",
                color: "var(--text-dim)",
                fontSize: 11.5,
                cursor: "pointer",
              }}
              // The box is a modifier for the click that follows, not a command of its own, so ticking it
              // must not close the menu the way choosing a value does.
              onMouseDown={(e) => e.stopPropagation()}
            >
              <input
                type="checkbox"
                checked={keep}
                onChange={(e) => setKeep(e.target.checked)}
                style={{ margin: 0, accentColor: "var(--accent)" }}
              />
              {keepLabel}
            </label>
          ) : null}
        </div>
      )}
    </div>
  );
}
