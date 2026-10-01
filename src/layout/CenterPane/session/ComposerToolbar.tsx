import { useEffect, useId, useLayoutEffect, useReducer, useRef, useState, type ReactNode } from "react";
import Icons from "../../../components/Icons";
import { useT } from "../../../i18n";
import type { ComposerChipId } from "../../../store/settings";
import { arrangeComposerChips, partitionComposerChips, type ComposerChip } from "./composerLayout";

/** Matches the flex gap of `.sv-controls-primary` in session-view.css. */
const CHIP_GAP = 6;

/**
 * The chips the user turned on stay beside the message in their chosen order; chips that are off are not
 * rendered anywhere. On the desktop the row is measured before paint, chips that do not fit fold into a
 * More row, and the More toggle exists only while something is folded. Mobile wraps all of them inline.
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
  const observed = useRef(new Set<Element>());
  const [, remeasure] = useReducer((n: number) => n + 1, 0);

  const { enabled } = arrangeComposerChips(chips, inline);
  const inlineChips = mobile || !layout.overflow ? enabled : enabled.slice(0, layout.inlineCount);
  const folded = mobile ? [] : enabled.slice(inlineChips.length);
  const showToggle = !mobile && folded.length > 0;
  const open = mobile || (showToggle && expanded);

  useLayoutEffect(() => {
    if (mobile) return;
    const ro = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(() => remeasure());
    observer.current = ro;
    const measure = () => remeasure();
    let active = true;
    window.addEventListener("resize", measure);
    document.fonts?.addEventListener("loadingdone", measure);
    void document.fonts?.ready.then(() => { if (active) measure(); });
    return () => {
      active = false;
      ro?.disconnect();
      observer.current = null;
      observed.current.clear();
      window.removeEventListener("resize", measure);
      document.fonts?.removeEventListener("loadingdone", measure);
    };
  }, [mobile]);

  // Folded chips remain measurable, but inert and invisible. Observe their natural width too, so font,
  // label and locale changes cannot leave stale cached widths behind the collapsed row.
  useLayoutEffect(() => {
    if (mobile || !primary.current || !box.current) return;
    const slots = box.current.querySelectorAll<HTMLElement>(".sv-chip-slot");
    const targets = new Set<Element>([primary.current, ...slots]);
    if (toggle.current) targets.add(toggle.current);
    for (const target of observed.current) if (!targets.has(target)) observer.current?.unobserve(target);
    for (const target of targets) if (!observed.current.has(target)) observer.current?.observe(target);
    observed.current = targets;
    const currentWidths = new Map<ComposerChipId, number>();
    for (const slot of slots) {
      // A narrow expanded row may clamp the visible chip. Measure its intrinsic width before paint,
      // then restore the clamp so it never overflows the composer or feeds a shrunken width back in.
      const previousWidth = slot.style.width;
      const previousMax = slot.style.maxWidth;
      slot.style.width = "max-content";
      slot.style.maxWidth = "none";
      const width = slot.getBoundingClientRect().width;
      slot.style.width = previousWidth;
      slot.style.maxWidth = previousMax;
      currentWidths.set(slot.dataset.chip as ComposerChipId, width);
    }
    widths.current = currentWidths;
    const probe = Math.max(toggle.current?.getBoundingClientRect().width ?? 0, toggle.current?.scrollWidth ?? 0);
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

  const slot = (chip: ComposerChip) => <div className="sv-chip-slot" style={{ maxWidth: "100%" }} data-chip={chip.id} key={chip.id}>{chip.node}</div>;

  return <div className="sv-controls sv-toolbar" ref={box} style={{ position: "relative" }}>
    <div className="sv-controls-main">
      <div className={mobile ? "sv-controls-primary sv-controls-primary-wrap" : "sv-controls-primary"} ref={primary}
        style={!mobile && showToggle ? { minWidth: `min(100%, ${moreWidth.current}px)` } : undefined}>
        {inlineChips.map(slot)}
        {!mobile && enabled.length > 0 && <button type="button" className="sv-chip sv-more-toggle" ref={toggle}
          aria-hidden={!showToggle || undefined} inert={!showToggle} tabIndex={showToggle ? undefined : -1}
          style={showToggle ? { maxWidth: "100%", overflow: "hidden" } : {
            position: "absolute", visibility: "hidden", pointerEvents: "none", width: "max-content",
          }}
          title={t("chat.moreOptions")} aria-label={t("chat.moreOptions")}
          aria-expanded={expanded} aria-controls={id} onClick={() => setExpanded(value => !value)}>
          <Icons.sliders size={14} />
          <span>{t("chat.moreOptions")}</span>
          <Icons.chevD size={12} />
        </button>}
      </div>
      <div className="sv-controls-actions">{actions}</div>
    </div>
    {folded.length > 0 && <div className="sv-controls-secondary" id={id} aria-hidden={!open || undefined} inert={!open}
      style={open ? undefined : { position: "absolute", visibility: "hidden", pointerEvents: "none", width: "100%", height: 0, padding: 0, overflow: "clip" }}>
      {folded.map(slot)}
    </div>}
    <div className="sv-controls-status">{status}</div>
  </div>;
}
