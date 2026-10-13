//! Narrow stand-in for a collapsed side panel. It keeps that panel's Show control in the corner where
//! its Hide control sat, so collapsing never moves the button to another part of the window.

import Icons from "../components/Icons";
import { useT } from "../i18n";
import { useTermStore } from "../store/termStore";

export function PanelRail({ side }: { side: "left" | "right" }) {
  const t = useT();
  const toggle = useTermStore((s) => (side === "left" ? s.toggleLeft : s.toggleRight));
  const label = t(side === "left" ? "titlebar.showLeft" : "titlebar.showRight");
  const Icon = side === "left" ? Icons.panelLeft : Icons.panel;
  return (
    <aside className={`col col-${side} col-rail`}>
      <div className="col-fill" />
      <div className="col-foot">
        <button className="icon-btn sm" title={label} aria-label={label} onClick={toggle}>
          <Icon size={14} />
        </button>
      </div>
    </aside>
  );
}
