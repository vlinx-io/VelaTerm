//! Shared pointer and keyboard menus. Pointer interaction preserves the editor's selection;
//! keyboard navigation moves focus into the menu and restores its invoking control on dismissal.
import { type ReactNode, type Ref, useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { useSuspendNativeViews } from "../hooks/nativeViewSuspend";

export interface MenuItem {
  label: string;
  onClick?: (e: React.MouseEvent) => void;
  /** A stable target for navigational items, including opening in another tab. */
  href?: string;
  danger?: boolean;
  separator?: boolean;
  disabled?: boolean;
  icon?: ReactNode;
  /** Submenus open on hover, click, or the right arrow key. */
  submenu?: MenuItem[];
  shortcut?: string;
  checked?: boolean;
}
const MARGIN = 8;
const enabled = (items: MenuItem[]) => items.flatMap((item, i) => item.separator || item.disabled ? [] : [i]);
const same = (a: number[], b: number[]) => a.length === b.length && a.every((n, i) => b[i] === n);
const branch = (items: MenuItem[], path: number[]) => path.reduce((list, i) => list[i]?.submenu ?? [], items);

export function ContextMenu({ x, y, items, onClose }: { x: number; y: number; items: MenuItem[]; onClose: () => void }) {
  useSuspendNativeViews();
  const id = useId(); const overlay = useRef<HTMLDivElement>(null); const panel = useRef<HTMLDivElement>(null);
  const origin = useRef(document.activeElement as HTMLElement | null);
  const [active, setActive] = useState<number[]>([]); const [open, setOpen] = useState<number[]>([]);
  const [pos, setPos] = useState({ left: x, top: y });
  const search = useRef({ text: "", time: 0 });
  const close = useCallback(() => {
    if (overlay.current?.contains(document.activeElement) && origin.current?.isConnected) origin.current.focus({ preventScroll: true });
    onClose();
  }, [onClose]);
  const move = (path: number[]) => {
    setActive(path);
    requestAnimationFrame(() => {
      const row = document.getElementById(`${id}-${path.join("-")}`);
      row?.focus({ preventScroll: true }); row?.scrollIntoView({ block: "nearest", inline: "nearest" });
    });
  };
  useEffect(() => {
    const keydown = (event: KeyboardEvent) => {
      if (event.isComposing) return;
      const key = event.key;
      const handled = () => { event.preventDefault(); event.stopPropagation(); };
      if (key === "Tab") { event.stopPropagation(); close(); return; }
      if (key === "Escape") {
        handled();
        if (open.length) { const parent = [...open]; setOpen(open.slice(0, -1)); move(parent); }
        else close();
        return;
      }
      const parent = active.length ? active.slice(0, -1) : [];
      const list = branch(items, parent); const choices = enabled(list); const current = active.at(-1) ?? -1;
      const item = list[current];
      if (["ArrowDown", "ArrowUp", "Home", "End"].includes(key)) {
        handled(); if (!choices.length) return;
        const index = choices.indexOf(current);
        const next = key === "Home" ? choices[0] : key === "End" ? choices.at(-1)! : choices[(index + (key === "ArrowDown" ? 1 : index < 0 ? 0 : -1) + choices.length) % choices.length];
        setOpen(parent); move([...parent, next]);
      } else if (key === "ArrowRight" || key === "Enter" || key === " ") {
        handled();
        if (!item) { if (choices.length) move([...parent, choices[0]]); return; }
        if (item.disabled) return;
        if (item.submenu) { setOpen(active); const first = enabled(item.submenu)[0]; if (first !== undefined) move([...active, first]); }
        else if (key !== "ArrowRight") document.getElementById(`${id}-${active.join("-")}`)?.click();
      } else if (key === "ArrowLeft") {
        handled(); if (parent.length) { setOpen(parent.slice(0, -1)); move(parent); }
      } else if (key.length === 1 && !event.metaKey && !event.ctrlKey && !event.altKey) {
        handled(); const now = Date.now(); const letter = key.toLocaleLowerCase();
        search.current = { text: now - search.current.time < 700 ? search.current.text + letter : letter, time: now };
        const query = [...search.current.text].every(c => c === letter) ? letter : search.current.text;
        const ordered = [...choices.filter(i => i > current), ...choices.filter(i => i <= current)];
        const found = ordered.find(i => list[i].label.trim().toLocaleLowerCase().startsWith(query));
        if (found !== undefined) { setOpen(parent); move([...parent, found]); }
      }
    };
    window.addEventListener("keydown", keydown, true);
    return () => window.removeEventListener("keydown", keydown, true);
  });
  useLayoutEffect(() => {
    if (!panel.current) return;
    const rect = panel.current.getBoundingClientRect();
    setPos({ left: Math.max(MARGIN, Math.min(x, innerWidth - rect.width - MARGIN)), top: Math.max(MARGIN, Math.min(y, innerHeight - rect.height - MARGIN)) });
  }, [x, y, items]);
  return <div ref={overlay} className="context-menu-layer" style={{ position: "fixed", inset: 0, zIndex: 1250 }} onClick={close} onContextMenu={e => { e.preventDefault(); close(); }}>
    <div style={{ position: "fixed", ...pos }}>
      <MenuPanel panelRef={panel} items={items} path={[]} active={active} open={open} id={id} setActive={setActive} setOpen={setOpen} onClose={close} />
    </div>
  </div>;
}

interface PanelProps {
  panelRef?: Ref<HTMLDivElement>; items: MenuItem[]; path: number[]; active: number[]; open: number[]; id: string;
  setActive: (path: number[]) => void; setOpen: (path: number[]) => void; onClose: () => void;
}
const panelStyle: React.CSSProperties = {
  background: "var(--bg-elevated)", border: "1px solid var(--border-strong)", borderRadius: 8, padding: 4,
  minWidth: 188, maxWidth: "calc(100vw - 16px)", maxHeight: "min(70vh, calc(100vh - 16px))", overflowY: "auto",
  boxShadow: "var(--shadow)",
};
function MenuPanel(props: PanelProps) {
  const { panelRef, items, path, active, open, id, setActive, setOpen, onClose } = props;
  const [anchor, setAnchor] = useState<DOMRect | null>(null);
  const aligned = items.some(i => i.icon || i.checked !== undefined);
  return <div ref={panelRef} role="menu" style={panelStyle} onClick={e => e.stopPropagation()}
    onMouseDown={e => e.preventDefault()}>
    {items.map((item, i) => {
      if (item.separator) return <div key={i} role="separator" style={{ height: 1, background: "var(--border)", margin: "5px 4px" }} />;
      const itemPath = [...path, i]; const isActive = same(active, itemPath); const expanded = item.submenu && open[path.length] === i && same(open.slice(0, path.length), path);
      const style: React.CSSProperties = { display: "flex", alignItems: "center", gap: 8, minHeight: 30, padding: "5px 8px", borderRadius: 5,
        color: item.disabled ? "var(--text-muted)" : item.danger ? "var(--status-error)" : "var(--text-primary)",
        opacity: item.disabled ? .45 : 1, cursor: item.disabled ? "default" : "pointer", textDecoration: "none", outline: "none",
        background: isActive ? "var(--accent-soft)" : undefined };
      const activate = (e: React.MouseEvent) => {
        if (item.disabled) { e.preventDefault(); return; }
        if (item.submenu) { e.preventDefault(); setAnchor(e.currentTarget.getBoundingClientRect()); setActive(itemPath); setOpen(expanded ? path : itemPath); return; }
        if (item.href && (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey)) { onClose(); return; }
        if (item.onClick) e.preventDefault(); onClose(); item.onClick?.(e);
      };
      const attrs = { id: `${id}-${itemPath.join("-")}`, className: "menu-item", role: item.checked !== undefined ? "menuitemcheckbox" : "menuitem", tabIndex: -1,
        "aria-disabled": item.disabled || undefined, "aria-checked": item.checked, "aria-haspopup": item.submenu ? "menu" as const : undefined,
        "aria-expanded": item.submenu ? !!expanded : undefined, style, onClick: activate,
        onMouseEnter: (e: React.MouseEvent<HTMLElement>) => {
          if (item.disabled) return;
          setActive(itemPath); setAnchor(e.currentTarget.getBoundingClientRect()); setOpen(item.submenu ? itemPath : path);
        },
        onFocus: (e: React.FocusEvent<HTMLElement>) => { setAnchor(e.currentTarget.getBoundingClientRect()); },
      };
      const content = <>{aligned && <span aria-hidden style={{ width: 16, flex: "0 0 16px", display: "grid", placeItems: "center", color: item.checked ? "var(--accent)" : "inherit" }}>{item.checked ? "✓" : item.icon}</span>}<span style={{ flex: 1, overflowWrap: "anywhere" }}>{item.label}</span>{item.shortcut && <span aria-hidden style={{ color: "var(--text-muted)", whiteSpace: "nowrap", paddingLeft: 24 }}>{item.shortcut}</span>}{item.submenu && <span aria-hidden style={{ color: "var(--text-muted)" }}>›</span>}</>;
      return <div key={i} onMouseLeave={() => { if (expanded) { setOpen(path); setActive(itemPath); } }}>
        {item.href && !item.submenu ? <a {...attrs} href={item.disabled ? undefined : item.href}>{content}</a> : <div {...attrs}>{content}</div>}
        {expanded && anchor && <Submenu {...props} panelRef={undefined} path={itemPath} items={item.submenu!} anchor={anchor} />}
      </div>;
    })}
  </div>;
}
function Submenu({ anchor, ...props }: PanelProps & { anchor: DOMRect }) {
  const ref = useRef<HTMLDivElement>(null); const [pos, setPos] = useState({ left: anchor.right, top: anchor.top - 4, flip: false });
  useLayoutEffect(() => {
    if (!ref.current) return;
    const { width, height } = ref.current.getBoundingClientRect();
    const flip = anchor.right + width + 8 > innerWidth - MARGIN;
    const left = flip ? anchor.left - width - 8 : anchor.right;
    setPos({ left: Math.max(MARGIN, Math.min(left, innerWidth - width - 8 - MARGIN)), top: Math.max(MARGIN, Math.min(anchor.top - 4, innerHeight - height - MARGIN)), flip });
  }, [anchor]);
  // Transparent padding bridges the visible gap so pointer travel does not dismiss the submenu.
  return <div style={{ position: "fixed", left: pos.left, top: pos.top, paddingLeft: pos.flip ? 0 : 8, paddingRight: pos.flip ? 8 : 0, zIndex: 1251 }}><MenuPanel {...props} panelRef={ref} /></div>;
}
