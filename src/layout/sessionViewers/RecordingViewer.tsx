//! Read-only terminal recording viewer. It streams backend recording bytes into an xterm with
//! disabled stdin, no PTY, and built-in terminal search.
//!
//! Global-search anchors prefill `initialQuery` and call findNext after replay until reaching
//! `scrollToOrdinal`; recording matches are located by ordinal count (see search.rs).
//!
//! For performance and stability, xterm rebuilds only when sessionId changes. Moving between matches
//! within a session uses incremental findNext/findPrevious in a separate effect. Rebuilding per match
//! was slow and triggered xterm teardown races where callbacks accessed dimensions after disposal.

import { useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { SearchAddon } from "@xterm/addon-search";
import type { WebglAddon } from "@xterm/addon-webgl";
import { loadWebglCtor } from "../../term/rendererAddons";
import { WebLinksAddon } from "@xterm/addon-web-links";

import Icons from "../../components/Icons";
import { useT } from "../../i18n";
import { readRecording } from "../../ipc/commands";
import { useTermStore } from "../../store/termStore";
import { terminalLinkHandler, webLinkActivate } from "../../terminal/openLink";
import { resolveTheme, XTERM_THEME } from "../../theme";

const SEARCH_OPTS = {
  decorations: {
    matchBackground: "#d29922",
    activeMatchBackground: "#f0883e",
    matchOverviewRuler: "#d29922",
    activeMatchColorOverviewRuler: "#f0883e",
  },
};

export function RecordingViewer({
  sessionId,
  initialQuery,
  scrollToOrdinal,
}: {
  sessionId: string;
  /** Initial search query for global-search location. */
  initialQuery?: string;
  /** One-based match ordinal reached through findNext after replay. */
  scrollToOrdinal?: number;
}) {
  const t = useT();
  const containerRef = useRef<HTMLDivElement | null>(null);
  const searchRef = useRef<SearchAddon | null>(null);
  const [query, setQuery] = useState(initialQuery ?? "");

  // Locate readiness and current ordinal for incremental movement within a session.
  const readyRef = useRef(false);
  const currentOrdinalRef = useRef(0);
  // Latest locate inputs, read by async callbacks without stale closures or rebuild-causing dependencies.
  const ordinalRef = useRef(scrollToOrdinal);
  const queryRef = useRef(initialQuery ?? "");
  ordinalRef.current = scrollToOrdinal;
  queryRef.current = initialQuery ?? "";

  // Create the terminal, load the recording, and locate initially. Rebuild only for session or locale changes.
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const term = new Terminal({
      fontFamily: 'Menlo, Monaco, "Courier New", monospace',
      fontSize: 13,
      lineHeight: 1.2,
      cursorBlink: false,
      disableStdin: true,
      // Archived sessions may be long; use a large scrollback to retain as much as possible.
      scrollback: 200000,
      // Handle OSC 8 links like usePtySession; see terminal/openLink.ts.
      linkHandler: terminalLinkHandler,
      theme: XTERM_THEME[resolveTheme(useTermStore.getState().theme)],
    });
    const fitAddon = new FitAddon();
    const searchAddon = new SearchAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(searchAddon);
    term.loadAddon(new WebLinksAddon(webLinkActivate));
    term.open(container);
    searchRef.current = searchAddon;
    readyRef.current = false;
    currentOrdinalRef.current = 0;

    let webgl: WebglAddon | undefined;
    let disposed = false;
    // The WebGL addon is a separate chunk (see term/rendererAddons.ts), so attaching is asynchronous
    // and the replay renders through DOM until it arrives. `disposed` keeps a late arrival from
    // attaching to a terminal this effect has already torn down.
    void loadWebglCtor().then((Ctor) => {
      if (!Ctor || disposed) return;
      try {
        webgl = new Ctor();
        term.loadAddon(webgl);
      } catch {
        /* Fall back to DOM rendering when WebGL is unavailable. */
      }
    });
    const safeFit = () => {
      if (disposed) return;
      try {
        fitAddon.fit();
      } catch {
        /* Ignore edge cases such as zero width. */
      }
    };
    safeFit();

    const resizeObserver = new ResizeObserver(() => {
      if (disposed || !container.offsetParent || container.clientWidth === 0) return;
      safeFit();
    });
    resizeObserver.observe(container);

    // Query fixed at terminal creation.
    const locateQuery = queryRef.current.trim();

    // Stream chunks into xterm, then locate an anchor or scroll to the top.
    readRecording(sessionId, (bytes) => {
      if (!disposed) term.write(bytes);
    })
      .then(() => {
        if (disposed) return;
        // The final write callback ensures xterm has parsed all preceding content before search.
        term.write(`\r\n\x1b[2m${t("archive.recordingEnd")}\x1b[0m\r\n`, () => {
          if (disposed) return;
          if (locateQuery) {
            const n = Math.max(1, ordinalRef.current ?? 1);
            for (let k = 0; k < n; k++) {
              searchRef.current?.findNext(locateQuery, SEARCH_OPTS);
            }
            currentOrdinalRef.current = n;
          } else {
            term.scrollToTop();
          }
          readyRef.current = true;
        });
      })
      .catch((e) => {
        if (!disposed)
          term.writeln(`\r\n\x1b[31m${t("archive.readRecordingFailed", String(e))}\x1b[0m`);
      });

    return () => {
      disposed = true;
      readyRef.current = false;
      resizeObserver.disconnect();
      searchRef.current = null;
      try {
        webgl?.dispose();
      } catch {
        /* Ignore known disposal-time errors. */
      }
      term.dispose();
    };
    // Intentionally depend only on sessionId/t; refs provide current locate inputs without rebuilding.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, t]);

  // Reposition incrementally when the ordinal changes within the same session.
  useEffect(() => {
    if (!readyRef.current) return; // Initial terminal setup handles location before replay is ready.
    const locateQuery = (initialQuery ?? "").trim();
    if (!locateQuery) return;
    const target = Math.max(1, scrollToOrdinal ?? 1);
    let cur = currentOrdinalRef.current;
    while (cur < target) {
      searchRef.current?.findNext(locateQuery, SEARCH_OPTS);
      cur++;
    }
    while (cur > target) {
      searchRef.current?.findPrevious(locateQuery, SEARCH_OPTS);
      cur--;
    }
    currentOrdinalRef.current = target;
  }, [scrollToOrdinal, initialQuery]);

  return (
    <div style={{ position: "absolute", inset: 0, display: "flex", flexDirection: "column" }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          padding: "6px 8px",
          borderBottom: "1px solid var(--border)",
        }}
      >
        <div className="box" style={{ display: "flex", alignItems: "center", gap: 6, flex: 1 }}>
          <Icons.search size={13} />
          <input
            placeholder={t("archive.searchRecording")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                if (e.shiftKey) searchRef.current?.findPrevious(query, SEARCH_OPTS);
                else searchRef.current?.findNext(query, SEARCH_OPTS);
              }
            }}
            style={{
              flex: 1,
              background: "transparent",
              border: "none",
              outline: "none",
              color: "var(--text-primary)",
              fontSize: 12.5,
            }}
          />
        </div>
        <button
          className="icon-btn sm"
          title={t("common.prev")}
          onClick={() => searchRef.current?.findPrevious(query, SEARCH_OPTS)}
        >
          <Icons.chevD size={14} style={{ transform: "rotate(180deg)" }} />
        </button>
        <button
          className="icon-btn sm"
          title={t("common.next")}
          onClick={() => searchRef.current?.findNext(query, SEARCH_OPTS)}
        >
          <Icons.chevD size={14} />
        </button>
      </div>
      <div style={{ flex: 1, position: "relative", minHeight: 0 }}>
        <div ref={containerRef} style={{ position: "absolute", inset: 0, padding: 6 }} />
      </div>
    </div>
  );
}
