//! Shared context menu with a full-screen dismissal backdrop and hover submenus. After rendering,
//! its measured position is clamped to the viewport so menus near the bottom or right stay visible.

import { type ReactNode, type Ref, useLayoutEffect, useRef, useState } from "react";
import { useSuspendNativeViews } from "../hooks/nativeViewSuspend";

export interface MenuItem {
  label: string;
  /**
   * Click callback receives the mouse event so actions can inspect modifiers such as Option/Alt.
   * Existing `() => void` handlers remain assignable because TypeScript permits fewer parameters.
   */
  onClick?: (e: React.MouseEvent) => void;
  /** A stable target for navigational items, including opening in another tab. */
  href?: string;
  danger?: boolean;
  separator?: boolean;
  disabled?: boolean;
  /** Optional leading icon, such as an agent-specific mark; omitted icons reserve no space. */
  icon?: ReactNode;
  /** When present, this is a non-clickable parent item that opens a submenu on hover. */
  submenu?: MenuItem[];
}

/** Shared fixed-size, centered wrapper for leading menu icons. */
function ItemIcon({ icon }: { icon: ReactNode }) {
  return <span style={{ display: "grid", flex: "none", color: "var(--text-secondary)" }}>{icon}</span>;
}

/** Minimum gap between a menu and the viewport edge. */
const MARGIN = 8;

export function ContextMenu({
  x,
  y,
  items,
  onClose,
}: {
  x: number;
  y: number;
  items: MenuItem[];
  onClose: () => void;
}) {
  // Suspend native browser views while the menu is visible so they cannot cover it.
  useSuspendNativeViews();
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number }>({
    left: x,
    top: y,
  });

  // Measure before paint and clamp viewport overflow so the full menu remains visible.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    let left = x;
    let top = y;
    if (left + width > vw - MARGIN) left = Math.max(MARGIN, vw - MARGIN - width);
    if (top + height > vh - MARGIN) top = Math.max(MARGIN, vh - MARGIN - height);
    setPos((p) => (p.left === left && p.top === top ? p : { left, top }));
  }, [x, y, items]);

  return (
    <div
      style={{ position: "fixed", inset: 0, zIndex: 1000 }}
      onClick={onClose}
      onContextMenu={(e) => {
        e.preventDefault();
        onClose();
      }}
    >
      <div style={{ position: "fixed", left: pos.left, top: pos.top }}>
        {/* Attach the ref to the actual min-width panel. Near the right edge, measuring the outer
            wrapper would return a width constrained by remaining space and prevent correct clamping. */}
        <MenuPanel panelRef={ref} items={items} onClose={onClose} />
      </div>
    </div>
  );
}

const panelStyle: React.CSSProperties = {
  background: "var(--bg-elevated)",
  border: "1px solid var(--border)",
  borderRadius: 6,
  padding: 4,
  minWidth: 160,
  maxHeight: "60vh",
  overflowX: "hidden",
  overflowY: "auto",
  boxShadow: "0 6px 24px rgba(0,0,0,0.4)",
};

function MenuPanel({
  panelRef,
  items,
  onClose,
}: {
  panelRef?: Ref<HTMLDivElement>;
  items: MenuItem[];
  onClose: () => void;
}) {
  const [openSub, setOpenSub] = useState<number | null>(null);
  // Record the parent row's viewport rectangle for Submenu positioning: open right, flip left when
  // needed, then clamp vertically. Fixed positioning avoids clipping by the panel's overflow rules.
  const [anchor, setAnchor] = useState<DOMRect | null>(null);

  return (
    <div
      ref={panelRef}
      style={panelStyle}
      onClick={(e) => e.stopPropagation()}
      // Preserve focus and the underlying DOM selection on pointerdown. Editor copy/cut/paste actions
      // depend on that selection; preventDefault blocks focus transfer without suppressing click.
      onMouseDown={(e) => e.preventDefault()}
    >
      {items.map((item, i) =>
        item.separator ? (
          <div
            key={i}
            style={{ height: 1, background: "var(--border)", margin: "4px 0" }}
          />
        ) : item.submenu ? (
          <div
            key={i}
            style={{ position: "relative" }}
            onMouseEnter={(e) => {
              setAnchor(e.currentTarget.getBoundingClientRect());
              setOpenSub(i);
            }}
            onMouseLeave={() => setOpenSub((s) => (s === i ? null : s))}
          >
            <div
              className="menu-item"
              style={{
                color: "var(--text-primary)",
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                gap: 12,
              }}
            >
              <span style={{ display: "flex", alignItems: "center", gap: 8 }}>
                {item.icon && <ItemIcon icon={item.icon} />}
                <span>{item.label}</span>
              </span>
              <span style={{ color: "var(--text-muted)", fontSize: 10 }}>▸</span>
            </div>
            {openSub === i && anchor && (
              <Submenu anchor={anchor} items={item.submenu} onClose={onClose} />
            )}
          </div>
        ) : item.href && !item.disabled ? (
          <a key={i} className="menu-item" href={item.href}
            style={{ color: "var(--text-primary)", display: "flex", alignItems: "center", gap: 8, textDecoration: "none" }}
            onClick={(e) => {
              if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
              e.preventDefault(); item.onClick?.(e); onClose();
            }}>
            {item.icon && <ItemIcon icon={item.icon} />}{item.label}
          </a>
        ) : (
          <div
            key={i}
            className="menu-item"
            style={{
              color: item.disabled
                ? "var(--text-muted)"
                : item.danger
                  ? "var(--status-error)"
                  : "var(--text-primary)",
              opacity: item.disabled ? 0.5 : 1,
              cursor: item.disabled ? "default" : "pointer",
            }}
            onClick={(e) => {
              if (item.disabled) return;
              item.onClick?.(e);
              onClose();
            }}
          >
            {item.icon ? (
              <span style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <ItemIcon icon={item.icon} />
                <span>{item.label}</span>
              </span>
            ) : (
              item.label
            )}
          </div>
        ),
      )}
    </div>
  );
}

/** Submenu that opens to the right, flips left when necessary, and clamps vertically. */
function Submenu({
  anchor,
  items,
  onClose,
}: {
  anchor: DOMRect;
  items: MenuItem[];
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number }>({
    left: anchor.right,
    top: anchor.top - 4,
  });

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    // Flip left when the right side is too narrow, then clamp if neither side fully fits.
    let left = anchor.right;
    if (left + width > vw - MARGIN) {
      left = anchor.left - width;
      if (left < MARGIN) left = Math.max(MARGIN, vw - MARGIN - width);
    }
    let top = anchor.top - 4;
    if (top + height > vh - MARGIN) top = Math.max(MARGIN, vh - MARGIN - height);
    setPos((p) => (p.left === left && p.top === top ? p : { left, top }));
  }, [anchor]);

  return (
    <div
      ref={ref}
      style={{ position: "fixed", left: pos.left, top: pos.top, zIndex: 1001 }}
    >
      <MenuPanel items={items} onClose={onClose} />
    </div>
  );
}
