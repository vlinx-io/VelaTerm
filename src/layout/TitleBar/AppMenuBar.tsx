//! Alt-triggered application menu bar for Windows and Linux.
//!
//! macOS already has a real menu bar in the system title bar (built in `src-tauri/src/lib.rs`), so this
//! row renders only on other platforms. It follows the VS Code "toggle" behaviour: hidden until a bare
//! Alt press, then usable with the mouse or the arrow keys, and hidden again by Alt or Escape.
//!
//! Only a bare Alt press-and-release toggles it. Any other key or a mouse button pressed while Alt is
//! held cancels the gesture, so Alt+key still reaches the terminal as a Meta sequence and a bare Alt,
//! which sends nothing to the pty, is the one press the menu can claim.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  effectiveCombo,
  formatCombo,
  type ShortcutAction,
} from "../../hooks/shortcutRegistry";
import { useT } from "../../i18n";
import { env, platform } from "../../platform";
import { useTermStore } from "../../store/termStore";
import { focusTerminal } from "../../terminal/registry";
import { runMenuAction } from "./appMenuActions";

const WEBSITE_URL = "https://velaterm.com";
const FEEDBACK_URL = "https://velaterm.com/feedback";

interface MenuItemDef {
  label: string;
  /** Right-aligned accelerator text, derived from the live binding rather than hardcoded. */
  accel?: string;
  run: () => void;
}

type MenuEntry = MenuItemDef | "separator";

interface MenuDef {
  title: string;
  entries: MenuEntry[];
}

export function AppMenuBar() {
  const t = useT();
  const shortcutOverrides = useTermStore((s) => s.shortcutOverrides);
  const [visible, setVisible] = useState(false);
  // Highlighted top-level menu; meaningful only while the bar is visible.
  const [cursor, setCursor] = useState(0);
  const [open, setOpen] = useState(false);
  const [activeItem, setActiveItem] = useState(-1);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const titleRefs = useRef<(HTMLButtonElement | null)[]>([]);
  // The Alt listener is registered once, so it reads visibility through a ref instead of a stale closure.
  const visibleRef = useRef(false);

  const accel = useCallback(
    (action: ShortcutAction) =>
      formatCombo(effectiveCombo(action, shortcutOverrides)),
    [shortcutOverrides],
  );

  const menus = useMemo<MenuDef[]>(
    () => [
      {
        title: t("menubar.file"),
        entries: [
          {
            label: `${t("settings.title")}…`,
            run: () => runMenuAction("settings"),
          },
          {
            label: `${t("updater.title")}…`,
            run: () => runMenuAction("check-update"),
          },
          "separator",
          {
            label: t("menubar.clearBadges"),
            run: () => runMenuAction("clear-badges"),
          },
        ],
      },
      {
        title: t("menubar.terminal"),
        entries: [
          {
            label: t("menubar.newTerminal"),
            accel: accel("newTab"),
            run: () => useTermStore.getState().newScratchTab(),
          },
          "separator",
          {
            label: t("term.splitRight"),
            accel: accel("splitRight"),
            run: () => runMenuAction("split-right"),
          },
          {
            label: t("term.splitDown"),
            accel: accel("splitDown"),
            run: () => runMenuAction("split-down"),
          },
        ],
      },
      {
        title: t("menubar.help"),
        entries: [
          {
            label: t("menubar.visitWebsite"),
            run: () => void platform.opener.openExternal(WEBSITE_URL),
          },
          {
            label: `${t("menubar.sendFeedback")}…`,
            run: () => void platform.opener.openExternal(FEEDBACK_URL),
          },
          "separator",
          {
            label: `❤️ ${t("titlebar.share")}…`,
            run: () => runMenuAction("share"),
          },
        ],
      },
    ],
    [t, accel],
  );

  useEffect(() => {
    visibleRef.current = visible;
  }, [visible]);

  // Hide the bar and hand the keyboard back to the terminal, which is where it came from.
  const hide = useCallback(() => {
    setVisible(false);
    setOpen(false);
    setActiveItem(-1);
    const id = useTermStore.getState().activeSessionId;
    if (id) focusTerminal(id);
  }, []);

  const show = useCallback(() => {
    setVisible(true);
    setCursor(0);
    setOpen(false);
    setActiveItem(-1);
  }, []);

  // Bare-Alt toggle. `armed` survives only while the Alt press stays alone: another key, a mouse
  // button, or the window losing focus cancels it, which keeps Alt+key combinations untouched.
  useEffect(() => {
    if (env.isMac) return;
    let armed = false;
    const onKeyDown = (e: KeyboardEvent) => {
      if (
        e.key === "Alt" &&
        !e.ctrlKey &&
        !e.metaKey &&
        !e.shiftKey &&
        !e.repeat
      ) {
        armed = true;
        // Browsers focus their own menu on a bare Alt; suppressing the default here keeps the press
        // ours. Modifier state is tracked by the OS, so Alt+key combinations are unaffected.
        e.preventDefault();
        return;
      }
      armed = false;
    };
    const onKeyUp = (e: KeyboardEvent) => {
      if (e.key !== "Alt") {
        armed = false;
        return;
      }
      if (!armed) return;
      armed = false;
      e.preventDefault();
      if (visibleRef.current) hide();
      else show();
    };
    const disarm = () => {
      armed = false;
    };
    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("keyup", onKeyUp, true);
    window.addEventListener("mousedown", disarm, true);
    window.addEventListener("blur", disarm);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("keyup", onKeyUp, true);
      window.removeEventListener("mousedown", disarm, true);
      window.removeEventListener("blur", disarm);
    };
  }, [hide, show]);

  // Keyboard focus follows the highlighted menu so arrow keys and Enter work without the mouse.
  useEffect(() => {
    if (visible) titleRefs.current[cursor]?.focus();
  }, [visible, cursor]);

  // A click elsewhere closes the dropdown but leaves the bar up, matching a native menu bar.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) {
        setOpen(false);
        setActiveItem(-1);
      }
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  const entries = menus[cursor]?.entries ?? [];
  // Separators are skipped when moving through a dropdown with the arrow keys.
  const step = (from: number, delta: number): number => {
    for (let i = 1; i <= entries.length; i++) {
      const idx =
        (from + delta * i + entries.length * i) % entries.length;
      if (entries[idx] !== "separator") return idx;
    }
    return -1;
  };

  const activate = (item: MenuItemDef) => {
    hide();
    item.run();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    const n = menus.length;
    switch (e.key) {
      case "Escape":
        e.preventDefault();
        if (open) {
          setOpen(false);
          setActiveItem(-1);
        } else hide();
        return;
      case "ArrowRight":
        e.preventDefault();
        setCursor((c) => (c + 1) % n);
        setActiveItem(-1);
        return;
      case "ArrowLeft":
        e.preventDefault();
        setCursor((c) => (c - 1 + n) % n);
        setActiveItem(-1);
        return;
      case "ArrowDown":
        e.preventDefault();
        if (!open) {
          setOpen(true);
          setActiveItem(step(-1, 1));
        } else setActiveItem((i) => step(i, 1));
        return;
      case "ArrowUp":
        e.preventDefault();
        if (open) setActiveItem((i) => step(i, -1));
        return;
      case "Enter":
      case " ": {
        e.preventDefault();
        const entry = open ? entries[activeItem] : undefined;
        if (entry && entry !== "separator") activate(entry);
        else {
          setOpen(true);
          setActiveItem(step(-1, 1));
        }
        return;
      }
    }
  };

  // macOS uses its native menu bar; browsers reached from a Mac keyboard follow the same convention.
  if (env.isMac || !visible) return null;

  return (
    <div className="menubar" ref={rootRef} onKeyDown={onKeyDown}>
      {menus.map((menu, i) => (
        <div className="menubar-item" key={menu.title}>
          <button
            ref={(el) => {
              titleRefs.current[i] = el;
            }}
            className={`menubar-title${open && cursor === i ? " open" : ""}`}
            onClick={() => {
              setActiveItem(-1);
              if (cursor === i) setOpen((o) => !o);
              else {
                setCursor(i);
                setOpen(true);
              }
            }}
            onMouseEnter={() => {
              if (open && cursor !== i) {
                setCursor(i);
                setActiveItem(-1);
              }
            }}
          >
            {menu.title}
          </button>
          {open && cursor === i && (
            <div className="menubar-dropdown" role="menu">
              {menu.entries.map((entry, j) =>
                entry === "separator" ? (
                  <div className="menubar-sep" key={`sep-${j}`} />
                ) : (
                  <button
                    key={entry.label}
                    role="menuitem"
                    className={`menubar-entry${activeItem === j ? " active" : ""}`}
                    onMouseEnter={() => setActiveItem(j)}
                    onClick={() => activate(entry)}
                  >
                    <span>{entry.label}</span>
                    {entry.accel && (
                      <span className="menubar-accel">{entry.accel}</span>
                    )}
                  </button>
                ),
              )}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
