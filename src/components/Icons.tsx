//! Hand-drawn outline icon set (16 px, currentColor stroke), based on port/components/Icons.jsx from the design handoff.
//! Has no external dependencies; access icons by name, for example <Icons.terminal size={14} />.

import React from "react";

export interface IconProps extends Omit<React.SVGProps<SVGSVGElement>, "fill"> {
  size?: number;
  /** Stroke width. */
  sw?: number;
  /** When true, fill rather than stroke the icon, overriding SVGProps' string-valued fill. */
  fill?: boolean;
}

type IconComponent = (props?: IconProps) => React.ReactElement;

const I =
  (paths: React.ReactNode, vb = "0 0 16 16"): IconComponent =>
  ({ size = 16, sw = 1.5, fill = false, ...p }: IconProps = {}) =>
    React.createElement(
      "svg",
      {
        width: size,
        height: size,
        viewBox: vb,
        fill: fill ? "currentColor" : "none",
        stroke: fill ? "none" : "currentColor",
        strokeWidth: sw,
        strokeLinecap: "round",
        strokeLinejoin: "round",
        ...p,
      },
      paths,
    );

const r = React.createElement;

const Icons: Record<string, IconComponent> = {
  chevR: I(r("path", { key: 1, d: "M6 4l4 4-4 4" })),
  chevD: I(r("path", { key: 1, d: "M4 6l4 4 4-4" })),
  folder: I(r("path", { key: 1, d: "M2 5.5A1.5 1.5 0 013.5 4h2.8l1.2 1.4h5A1.5 1.5 0 0114 6.9V11A1.5 1.5 0 0112.5 12.5h-9A1.5 1.5 0 012 11z" })),
  folderOpen: I([r("path", { key: 1, d: "M2 5.5A1.5 1.5 0 013.5 4h2.8l1.2 1.4h5A1.5 1.5 0 0114 6.9" }), r("path", { key: 2, d: "M2.2 7h11.3l-1.2 4.6a1 1 0 01-1 .9H3.5a1 1 0 01-1-.74z" })]),
  folderPlus: I([r("path", { key: 1, d: "M2 5.5A1.5 1.5 0 013.5 4h2.8l1.2 1.4h5A1.5 1.5 0 0114 6.9V11A1.5 1.5 0 0112.5 12.5h-9A1.5 1.5 0 012 11z" }), r("path", { key: 2, d: "M8 7.3v3.4M6.3 9h3.4" })]),
  home: I([r("path", { key: 1, d: "M2.5 7.6L8 3.2l5.5 4.4" }), r("path", { key: 2, d: "M4 7v5.3a.5.5 0 00.5.5h7a.5.5 0 00.5-.5V7" })]),
  clock: I([r("circle", { key: 1, cx: 8, cy: 8, r: 5.2 }), r("path", { key: 2, d: "M8 4.9v3.3l2.3 1.3" })]),
  project: I([r("rect", { key: 1, x: 2.5, y: 2.5, width: 11, height: 11, rx: 2 }), r("path", { key: 2, d: "M2.5 6h11" }), r("circle", { key: 3, cx: 4.6, cy: 4.3, r: 0.4, fill: "currentColor", stroke: "none" })]),
  terminal: I([r("rect", { key: 1, x: 2, y: 3, width: 12, height: 10, rx: 1.5 }), r("path", { key: 2, d: "M4.8 6.5L6.8 8l-2 1.5M8.5 9.7h2.7" })]),
  bot: I([r("rect", { key: 1, x: 3, y: 5.5, width: 10, height: 7, rx: 2 }), r("path", { key: 2, d: "M8 3v2.5M5.5 8.6h.01M10.5 8.6h.01" }), r("path", { key: 3, d: "M3 9.2H1.8M14.2 9.2H13" })]),
  file: I([r("path", { key: 1, d: "M4 2.5h5L12 5.5V13a.5.5 0 01-.5.5h-7A.5.5 0 014 13z" }), r("path", { key: 2, d: "M8.8 2.6V5.5H11.7" })]),
  filePlus: I([r("path", { key: 1, d: "M4 2.5h5L12 5.5V13a.5.5 0 01-.5.5h-7A.5.5 0 014 13z" }), r("path", { key: 2, d: "M8.8 2.6V5.5H11.7" }), r("path", { key: 3, d: "M8 8v3.4M6.3 9.7h3.4" })]),
  image: I([r("rect", { key: 1, x: 2.5, y: 3, width: 11, height: 10, rx: 1.5 }), r("circle", { key: 2, cx: 5.8, cy: 6.2, r: 1.1 }), r("path", { key: 3, d: "M2.5 11l3.2-3 2.3 2.2 2.7-2.7 2.8 2.8" })]),
  search: I([r("circle", { key: 1, cx: 7, cy: 7, r: 4 }), r("path", { key: 2, d: "M10 10l3 3" })]),
  bell: I([r("path", { key: 1, d: "M12.5 11H3.5c1-1 1.3-2 1.3-3.5v-1A3.2 3.2 0 018 3.3a3.2 3.2 0 013.2 3.2v1c0 1.5.3 2.5 1.3 3.5z" }), r("path", { key: 2, d: "M6.8 13a1.3 1.3 0 002.4 0" })]),
  bellOff: I([r("path", { key: 1, d: "M12.5 11H3.5c1-1 1.3-2 1.3-3.5v-1A3.2 3.2 0 018 3.3a3.2 3.2 0 013.2 3.2v1c0 1.5.3 2.5 1.3 3.5z" }), r("path", { key: 2, d: "M6.8 13a1.3 1.3 0 002.4 0" }), r("path", { key: 3, d: "M3 2.5l10 11" })]),
  minus: I(r("path", { key: 1, d: "M3.5 8h9" })),
  plus: I(r("path", { key: 1, d: "M8 3.5v9M3.5 8h9" })),
  x: I(r("path", { key: 1, d: "M4 4l8 8M12 4l-8 8" })),
  splitV: I([r("rect", { key: 1, x: 2.5, y: 2.5, width: 11, height: 11, rx: 1.5 }), r("path", { key: 2, d: "M8 2.5v11" })]),
  splitH: I([r("rect", { key: 1, x: 2.5, y: 2.5, width: 11, height: 11, rx: 1.5 }), r("path", { key: 2, d: "M2.5 8h11" })]),
  close: I(r("path", { key: 1, d: "M4.5 4.5l7 7M11.5 4.5l-7 7" })),
  git: I([r("circle", { key: 1, cx: 4.5, cy: 4, r: 1.6 }), r("circle", { key: 2, cx: 4.5, cy: 12, r: 1.6 }), r("circle", { key: 3, cx: 11.5, cy: 6.5, r: 1.6 }), r("path", { key: 4, d: "M4.5 5.6v4.8M4.5 8h3.4A2 2 0 009.9 6.1" })]),
  sun: I([r("circle", { key: 1, cx: 8, cy: 8, r: 3 }), r("path", { key: 2, d: "M8 1.5v1.6M8 12.9v1.6M1.5 8h1.6M12.9 8h1.6M3.4 3.4l1.1 1.1M11.5 11.5l1.1 1.1M12.6 3.4l-1.1 1.1M4.5 11.5l-1.1 1.1" })]),
  moon: I(r("path", { key: 1, d: "M13 9.5A5.5 5.5 0 016.5 3a5.5 5.5 0 100 11 5.5 5.5 0 006.5-4.5z" })),
  sliders: I([r("path", { key: 1, d: "M3 5h7M3 11h3" }), r("circle", { key: 2, cx: 12, cy: 5, r: 1.6 }), r("circle", { key: 3, cx: 8, cy: 11, r: 1.6 })]),
  gear: I([r("circle", { key: 1, cx: 8, cy: 8, r: 4.3 }), r("circle", { key: 2, cx: 8, cy: 8, r: 1.7 }), r("path", { key: 3, d: "M8 1.6v2.1M8 12.3v2.1M1.6 8h2.1M12.3 8h2.1M3.47 3.47L5 5M11 11l1.53 1.53M12.53 3.47L11 5M5 11l-1.53 1.53" })]),
  share: I([r("circle", { key: 1, cx: 11.5, cy: 4, r: 1.9 }), r("circle", { key: 2, cx: 4.5, cy: 8, r: 1.9 }), r("circle", { key: 3, cx: 11.5, cy: 12, r: 1.9 }), r("path", { key: 4, d: "M6.15 7.05l3.7-2.1M6.15 8.95l3.7 2.1" })]),
  panel: I([r("rect", { key: 1, x: 2.5, y: 3, width: 11, height: 10, rx: 1.5 }), r("path", { key: 2, d: "M10 3v10" })]),
  panelFill: I([r("rect", { key: 1, x: 2.5, y: 3, width: 11, height: 10, rx: 1.5 }), r("path", { key: 2, d: "M10 3v10" }), r("rect", { key: 3, x: 10.5, y: 3.75, width: 2.25, height: 8.5, rx: 0.5, fill: "currentColor", stroke: "none" })]),
  panelLeft: I([r("rect", { key: 1, x: 2.5, y: 3, width: 11, height: 10, rx: 1.5 }), r("path", { key: 2, d: "M6 3v10" })]),
  panelLeftFill: I([r("rect", { key: 1, x: 2.5, y: 3, width: 11, height: 10, rx: 1.5 }), r("path", { key: 2, d: "M6 3v10" }), r("rect", { key: 3, x: 3.25, y: 3.75, width: 2.25, height: 8.5, rx: 0.5, fill: "currentColor", stroke: "none" })]),
  branch: I([r("circle", { key: 1, cx: 4, cy: 4, r: 1.4 }), r("circle", { key: 2, cx: 4, cy: 12, r: 1.4 }), r("circle", { key: 3, cx: 12, cy: 4, r: 1.4 }), r("path", { key: 4, d: "M4 5.4v5.2M12 5.4v1.1A3.5 3.5 0 018.5 10H6" })]),
  info: I([r("circle", { key: 1, cx: 8, cy: 8, r: 5.5 }), r("path", { key: 2, d: "M8 7.3v3.4M8 5.4h.01" })]),
  rename: I([r("path", { key: 1, d: "M3 11.5l6.4-6.4 2 2L5 13.5H3z" }), r("path", { key: 2, d: "M9.4 5.1l1.5-1.5a1 1 0 011.4 0l.6.6a1 1 0 010 1.4l-1.5 1.5" })]),
  trash: I([r("path", { key: 1, d: "M3.5 4.5h9M6 4.5V3.2A.7.7 0 016.7 2.5h2.6a.7.7 0 01.7.7V4.5M5 4.5l.5 8h5l.5-8" })]),
  dup: I([r("rect", { key: 1, x: 5, y: 5, width: 8, height: 8, rx: 1.5 }), r("path", { key: 2, d: "M3 10V4.5A1.5 1.5 0 014.5 3H10" })]),
  newGroup: I([r("path", { key: 1, d: "M2 5.5A1.5 1.5 0 013.5 4h2.8l1.2 1.4h5A1.5 1.5 0 0114 6.9V8" }), r("path", { key: 2, d: "M11.5 10v4M9.5 12h4" })]),
  dotsV: I([r("circle", { key: 1, cx: 8, cy: 3.5, r: 1, fill: "currentColor", stroke: "none" }), r("circle", { key: 2, cx: 8, cy: 8, r: 1, fill: "currentColor", stroke: "none" }), r("circle", { key: 3, cx: 8, cy: 12.5, r: 1, fill: "currentColor", stroke: "none" })]),
  check: I(r("path", { key: 1, d: "M3.5 8.5l3 3 6-7" })),
  cpu: I([r("rect", { key: 1, x: 4, y: 4, width: 8, height: 8, rx: 1 }), r("path", { key: 2, d: "M6.5 2.5v1.5M9.5 2.5v1.5M6.5 12v1.5M9.5 12v1.5M2.5 6.5H4M2.5 9.5H4M12 6.5h1.5M12 9.5h1.5" })]),
  copy: I([r("rect", { key: 1, x: 5.5, y: 5.5, width: 7.5, height: 7.5, rx: 1.5 }), r("path", { key: 2, d: "M3 10.2V4.2A1.2 1.2 0 014.2 3H10" })]),
  collapse: I([r("path", { key: 1, d: "M5.5 3.5L8 6l2.5-2.5M5.5 12.5L8 10l2.5 2.5M3.5 8h9" })]),
  expand: I([r("path", { key: 1, d: "M8 2.5L5.5 5h5zM8 13.5L5.5 11h5zM3.5 8h9" })]),
  reveal: I([r("path", { key: 1, d: "M2 5.5A1.5 1.5 0 013.5 4h2.8l1.2 1.4h5A1.5 1.5 0 0114 6.9V11A1.5 1.5 0 0112.5 12.5h-9A1.5 1.5 0 012 11z" })]),
  eye: I([r("path", { key: 1, d: "M1.7 8S4.2 3.8 8 3.8 14.3 8 14.3 8 11.8 12.2 8 12.2 1.7 8 1.7 8z" }), r("circle", { key: 2, cx: 8, cy: 8, r: 1.9 })]),
  eyeOff: I([r("path", { key: 1, d: "M1.7 8S4.2 3.8 8 3.8 14.3 8 14.3 8 11.8 12.2 8 12.2 1.7 8 1.7 8z" }), r("circle", { key: 2, cx: 8, cy: 8, r: 1.9 }), r("path", { key: 3, d: "M3 3l10 10" })]),
  restart: I([r("path", { key: 1, d: "M12.5 8a4.5 4.5 0 10-1.3 3.2" }), r("path", { key: 2, d: "M12.5 4.5V8H9" })]),
  globe: I([r("circle", { key: 1, cx: 8, cy: 8, r: 5.5 }), r("path", { key: 2, d: "M2.5 8h11M8 2.5c1.6 1.5 2.5 3.4 2.5 5.5S9.6 12.5 8 13.5C6.4 12 5.5 10.1 5.5 8S6.4 3.5 8 2.5z" })]),
  archive: I([r("rect", { key: 1, x: 2.5, y: 3, width: 11, height: 2.6, rx: 0.6 }), r("path", { key: 2, d: "M3.6 5.6h8.8V12a1 1 0 01-1 1H4.6a1 1 0 01-1-1z" }), r("path", { key: 3, d: "M6.5 8h3" })]),
  connect: I([r("path", { key: 1, d: "M2 8h5.5" }), r("path", { key: 2, d: "M5.5 5.5L8 8 5.5 10.5" }), r("rect", { key: 3, x: 10, y: 3, width: 4, height: 10, rx: 1 })]),
  monitor: I([r("rect", { key: 1, x: 2.5, y: 3, width: 11, height: 8, rx: 1.5 }), r("path", { key: 2, d: "M6 13.5h4M8 11v2.5" })]),
  download: I([r("path", { key: 1, d: "M8 2.5V10M4.8 7.2L8 10.4l3.2-3.2" }), r("path", { key: 2, d: "M3 11.2v1.3a1 1 0 001 1h8a1 1 0 001-1v-1.3" })]),
  // Mirror of download: same tray, arrow pointing up and out.
  upload: I([r("path", { key: 1, d: "M8 10.4V2.9M4.8 6.1L8 2.9l3.2 3.2" }), r("path", { key: 2, d: "M3 11.2v1.3a1 1 0 001 1h8a1 1 0 001-1v-1.3" })]),
  arrowLeft: I([r("path", { key: 1, d: "M13 8H3.5M7.5 4L3.5 8l4 4" })]),
  arrowRight: I([r("path", { key: 1, d: "M3 8h9.5M8.5 4l4 4-4 4" })]),
  external: I([r("path", { key: 1, d: "M6.5 3.5H4A1.5 1.5 0 002.5 5v7A1.5 1.5 0 004 13.5h7A1.5 1.5 0 0012.5 12V9.5" }), r("path", { key: 2, d: "M9.5 2.5h4v4M13.2 2.8L8 8" })]),
  compass: I([r("circle", { key: 1, cx: 8, cy: 8, r: 5.5 }), r("path", { key: 2, d: "M10.3 5.7L9 9 5.7 10.3 7 7z" })]),
  // Note: the Codex icon now uses the official OpenAI logo embedded manually in components/brandIcons.tsx and
  // shared by the Codex branch of sessionMeta.kindIconEl and usageBrandIconEl. The old three-ellipse placeholder
  // has been removed.

  // ── File-type icons, colored by extension in the right-panel Files tab; see RightPanel/fileIcons.tsx. ──
  code: I(r("path", { key: 1, d: "M6 4.5L2.5 8 6 11.5M10 4.5L13.5 8 10 11.5" })),
  braces: I([
    r("path", { key: 1, d: "M6.5 3C5.3 3 5 3.7 5 4.7c0 1.1 0 1.7-1 2.1 1 .4 1 1 1 2.1C5 12 5.3 13 6.5 13" }),
    r("path", { key: 2, d: "M9.5 3c1.2 0 1.5.7 1.5 1.7 0 1.1 0 1.7 1 2.1-1 .4-1 1-1 2.1 0 1-.3 2.1-1.5 2.1" }),
  ]),
  hash: I(r("path", { key: 1, d: "M6 3l-1.3 10M11 3l-1.3 10M3.4 6.3h9.2M2.9 9.7h9.2" })),
  lock: I([
    r("rect", { key: 1, x: 3.5, y: 7, width: 9, height: 6, rx: 1.2 }),
    r("path", { key: 2, d: "M5.5 7V5.3a2.5 2.5 0 015 0V7" }),
  ]),
  database: I([
    r("ellipse", { key: 1, cx: 8, cy: 4, rx: 4.5, ry: 1.8 }),
    r("path", { key: 2, d: "M3.5 4v8c0 1 2 1.8 4.5 1.8s4.5-.8 4.5-1.8V4" }),
    r("path", { key: 3, d: "M3.5 8c0 1 2 1.8 4.5 1.8s4.5-.8 4.5-1.8" }),
  ]),
  font: I(r("path", { key: 1, d: "M4 12.5l4-9.5 4 9.5M5.4 9.2h5.2" })),
  docLines: I([
    r("path", { key: 1, d: "M4 2.5h5L12 5.5V13a.5.5 0 01-.5.5h-7A.5.5 0 014 13z" }),
    r("path", { key: 2, d: "M8.8 2.6V5.5H11.7" }),
    r("path", { key: 3, d: "M5.8 8.4h4.4M5.8 10.4h3" }),
  ]),
  print: I([
    r("path", { key: 1, d: "M5 5V2.5h6V5" }),
    r("rect", { key: 2, x: 2, y: 5, width: 12, height: 5, rx: 1 }),
    r("path", { key: 3, d: "M5 10v3.5h6V10" }),
    r("circle", { key: 4, cx: 11.5, cy: 7.5, r: 0.7, fill: "currentColor", stroke: "none" }),
  ]),
};

export default Icons;
export { Icons };
