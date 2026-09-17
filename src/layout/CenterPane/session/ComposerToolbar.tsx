import { useEffect, useId, useLayoutEffect, useReducer, useRef, useState, type ReactNode } from "react";
import Icons from "../../../components/Icons";
import { useT } from "../../../i18n";
import type { ComposerChipId } from "../../../store/settings";
import { arrangeComposerChips, partitionComposerChips, type ComposerChip } from "./composerLayout";

/** Matches the flex gap of `.sv-controls-primary` in session-view.css. */
const CHIP_GAP = 6;

/**
 * The chips the user turned on stay beside the message in their chosen order. On the desktop the row is
 * measured before paint; chips that do not fit fold into a More row together with the chips that are
 * off, and the More toggle exists only while something is folded. Mobile keeps both rows visible.
 */
export function ComposerToolbar({ chips, inline, actions, status, mobile }: {
  chips: ComposerChip[];
  inline: ComposerChipId[];
  actions: ReactNode;
  status: ReactNode;
  mobile: boolean;
}) {
  const t = useT();
  const id = useId();
  const [expanded, setExpanded] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  const primary = useRef<HTMLDivElement>(null);
  const toggle = useRef<HTMLButtonElement>(null);
  // Everything inline plus the toggle as a probe until the first measurement corrects it before paint.
  const [layout, setLayout] = useState({ inlineCount: Infinity, overflow: true });
  const widths = useRef(new Map<ComposerChipId, number>());
  const moreWidth = useRef(0);
  const observer = useRef<ResizeObserver | null>(null);
  const [, remeasure] = useReducer((n: number) => n + 1, 0);

  const { enabled, hidden } = arrangeComposerChips(chips, inline);
  const inlineChips = mobile || !layout.overflow ? enabled : enabled.slice(0, layout.inlineCount);
  const folded = mobile ? hidden : [...enabled.slice(inlineChips.length), ...hidden];
  const showToggle = !mobile && layout.overflow;
  const open = mobile || (showToggle && expanded);

  useEffect(() => {
    if (mobile || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => remeasure());
    observer.current = ro;
    return () => {
      ro.disconnect();
      observer.current = null;
    };
  }, [mobile]);

  // Runs after every commit: cache the width of each rendered chip, then decide how many fit. A chip that
  // is not measured yet counts as fitting so it is rendered once, measured here, and moved before paint.
  useLayoutEffect(() => {
    if (mobile || !primary.current || !box.current) return;
    const slots = box.current.querySelectorAll<HTMLElement>(".sv-chip-slot");
    for (const slot of slots) {
      const width = slot.getBoundingClientRect().width;
      if (width > 0) widths.current.set(slot.dataset.chip as ComposerChipId, width);
      observer.current?.observe(slot);
    }
    observer.current?.observe(primary.current);
    const probe = toggle.current?.getBoundingClientRect().width ?? 0;
    if (probe > 0) moreWidth.current = probe;
    const next = partitionComposerChips(
      enabled.map((chip) => widths.current.get(chip.id) ?? 0),
      primary.current.getBoundingClientRect().width,
      moreWidth.current,
      CHIP_GAP,
    );
    if (next.inlineCount !== layout.inlineCount || next.overflow !== layout.overflow) setLayout(next);
    if (!next.overflow && expanded) setExpanded(false);
  });

  useEffect(() => {
    if (!expanded || mobile) return;
    const outside = (event: PointerEvent) => {
      if (!box.current?.contains(event.target as Node)) setExpanded(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || !box.current?.closest(".sv-composer")?.contains(event.target as Node)) return;
      // An open selector handles its own Escape first. A later Escape folds the settings row.
      if (box.current.querySelector('[role="option"], .sv-popover')) return;
      event.preventDefault();
      event.stopPropagation();
      setExpanded(false);
      toggle.current?.focus();
    };
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("keydown", escape, true);
    return () => {
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("keydown", escape, true);
    };
  }, [expanded, mobile]);

  const slot = (chip: ComposerChip) => <div className="sv-chip-slot" data-chip={chip.id} key={chip.id}>{chip.node}</div>;

  return <div className="sv-controls sv-toolbar" ref={box}>
    <div className="sv-controls-main">
      <div className={mobile ? "sv-controls-primary sv-controls-primary-wrap" : "sv-controls-primary"} ref={primary}>
        {inlineChips.map(slot)}
        {showToggle && <button type="button" className="sv-chip sv-more-toggle" ref={toggle}
          title={t("chat.moreOptions")} aria-label={t("chat.moreOptions")}
          aria-expanded={expanded} aria-controls={id} onClick={() => setExpanded(value => !value)}>
          <Icons.sliders size={14} />
          <span>{t("chat.moreOptions")}</span>
          <Icons.chevD size={12} />
        </button>}
      </div>
      <div className="sv-controls-actions">{actions}</div>
    </div>
    {(mobile || showToggle) && <div className="sv-controls-secondary" id={id} hidden={!open}>
      {folded.map(slot)}
    </div>}
    <div className="sv-controls-status">{status}</div>
  </div>;
}
