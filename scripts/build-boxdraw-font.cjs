#!/usr/bin/env node
// Build src/assets/fonts/jetbrains-mono-boxdraw-{400,700}.woff2: box drawing U+2500-257F and block elements
// U+2580-259F subset from the official JetBrains Mono TTFs, with the ink of every glyph that touches the left or
// right edge of its advance box pushed outward by EDGE font units (0.04 em, about half a pixel at 13 px).
//
// Why the overdraw: xterm's DOM renderer positions cells at fractional pixel offsets whenever the measured
// advance is not an integer. Two glyphs that meet exactly at such a boundary are each antialiased on their own
// and composited, which leaves a faint darker column at every cell edge even when the font metrics match
// perfectly. Overshooting the edge makes neighbours overlap by a hair and the seam disappears. The advance
// width is untouched, so cell measurement and letter-spacing are unaffected.
//
// Usage (one-off, from any scratch directory; these packages are intentionally not project dependencies):
//   npm install opentype.js subset-font
//   node <repo>/scripts/build-boxdraw-font.cjs <dir containing JetBrainsMono-Regular.ttf and -Bold.ttf> <repo>/src/assets/fonts
// Source TTFs: https://github.com/JetBrains/JetBrainsMono/releases (fonts/ttf/ inside the zip), OFL 1.1.

const fs = require("fs");
const path = require("path");
const opentype = require("opentype.js");
const subsetFont = require("subset-font");

const EDGE = 40;
const LO = 0x2500;
const HI = 0x259f;
const [srcDir, outDir] = process.argv.slice(2);

/** Zero head.created and head.modified (opentype.js stamps both with the build time) so rebuilds are byte-identical. */
function zeroHeadTimestamps(otf) {
  const numTables = otf.readUInt16BE(4);
  for (let i = 0; i < numTables; i++) {
    const rec = 12 + i * 16;
    if (otf.toString("latin1", rec, rec + 4) === "head") {
      const off = otf.readUInt32BE(rec + 8);
      otf.fill(0, off + 20, off + 36);
      return otf;
    }
  }
  throw new Error("head table not found");
}
if (!srcDir || !outDir) {
  console.error("usage: build-boxdraw-font.cjs <jetbrains-mono ttf dir> <output dir>");
  process.exit(2);
}

(async () => {
  let text = "";
  for (let cp = LO; cp <= HI; cp++) text += String.fromCodePoint(cp);
  for (const [weight, style] of [[400, "Regular"], [700, "Bold"]]) {
    const file = path.join(srcDir, `JetBrainsMono-${style}.ttf`);
    const src = opentype.parse(fs.readFileSync(file).buffer.slice(0));
    const glyphs = [
      new opentype.Glyph({ name: ".notdef", unicode: 0, advanceWidth: 600, path: new opentype.Path() }),
    ];
    for (let cp = LO; cp <= HI; cp++) {
      const g = src.charToGlyph(String.fromCodePoint(cp));
      if (!g || g.name === ".notdef") continue;
      const adv = g.advanceWidth;
      const push = (x) => (x <= 0 ? x - EDGE : x >= adv ? x + EDGE : x);
      const out = new opentype.Path();
      // getPath returns screen coordinates (y down); flip y back to font units.
      for (const c of g.getPath(0, 0, src.unitsPerEm).commands) {
        const n = { ...c };
        for (const k of ["x", "x1", "x2"]) if (n[k] !== undefined) n[k] = push(n[k]);
        for (const k of ["y", "y1", "y2"]) if (n[k] !== undefined) n[k] = -n[k];
        out.commands.push(n);
      }
      glyphs.push(new opentype.Glyph({ name: g.name || `u${cp.toString(16)}`, unicode: cp, advanceWidth: adv, path: out }));
    }
    const font = new opentype.Font({
      familyName: "JetBrains Mono", styleName: style, unitsPerEm: src.unitsPerEm,
      ascender: src.ascender, descender: src.descender, glyphs,
    });
    const otf = zeroHeadTimestamps(Buffer.from(font.toArrayBuffer()));
    const woff2 = await subsetFont(otf, text, { targetFormat: "woff2" });
    const outFile = path.join(outDir, `jetbrains-mono-boxdraw-${weight}.woff2`);
    fs.writeFileSync(outFile, woff2);
    console.log(`${outFile}: ${woff2.length} bytes, ${glyphs.length - 1} glyphs`);
  }
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
