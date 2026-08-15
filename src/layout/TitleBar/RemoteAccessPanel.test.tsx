//! Coverage for the advertised-IP selector (choosing an IP persists it and regenerates the pairing
//! link with exactly that address, a vanished persisted IP falls back to automatic, and the URL
//! ordering helper matches hosts exactly) and for restart persistence (the port field prefills the
//! last persisted port instead of the hardcoded 8799, and a failed auto-start surfaces its error).

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { WebServerStatus } from "../../ipc/webServer";

const {
  invokeMock,
  defaultInvoke,
  networkInterfacesListMock,
  webPairingCreateMock,
  webServerStatusMock,
  webServerStartMock,
  appSettings,
} = vi.hoisted(() => {
  const appSettings: Record<string, string> = {};
  // Default invoke implementation; tests overriding it (e.g. to defer get_app_settings) rely on
  // afterEach restoring it, because vi.clearAllMocks keeps implementations.
  const defaultInvoke = (cmd: string, args?: { entries?: Record<string, string> }) => {
    if (cmd === "get_app_settings") return Promise.resolve({ ...appSettings });
    if (cmd === "set_app_settings") {
      Object.assign(appSettings, args?.entries ?? {});
      return Promise.resolve(null);
    }
    return Promise.reject(new Error(`unexpected invoke: ${cmd}`));
  };
  return {
    appSettings,
    defaultInvoke,
    invokeMock: vi.fn(defaultInvoke),
    networkInterfacesListMock: vi.fn(),
    webPairingCreateMock: vi.fn(),
    webServerStatusMock: vi.fn(),
    webServerStartMock: vi.fn(),
  };
});

vi.mock("../../i18n", () => ({
  // Echo keys so assertions are locale-independent.
  useT: () => (key: string) => key,
}));
vi.mock("../../ipc/transport", () => ({ invoke: invokeMock }));
vi.mock("../../ipc/info", () => ({ copyText: vi.fn() }));
// The backdrop shell pulls in platform hooks irrelevant here; render children directly.
vi.mock("../../components/Backdrop", () => ({
  Backdrop: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));
// Stub the QR renderer to expose its value prop, so tests can assert the QR encodes exactly the
// pairing URL for the chosen host (AC 3: link AND QR) instead of inferring it from the copy button.
vi.mock("qrcode.react", () => ({
  QRCodeSVG: ({ value }: { value: string }) => <svg data-qr-value={value} />,
}));
vi.mock("../../ipc/webServer", () => ({
  networkInterfacesList: networkInterfacesListMock,
  webServerStatus: webServerStatusMock,
  webServerStart: webServerStartMock,
  webServerStop: vi.fn(),
  webPairingCreate: webPairingCreateMock,
  webDevicesList: vi.fn().mockResolvedValue([]),
  webDeviceRevoke: vi.fn(),
}));

import {
  orderUrlsBySelectedIp,
  RemoteAccessPanel,
  urlsForSelectedIp,
} from "./RemoteAccessPanel";

const LAN_URL = "https://192.168.1.5:8799";
const CGNAT_URL = "https://100.100.83.2:8799";

/** Running LanTls-like status listing a LAN URL first and the CGNAT (Tailscale) URL last. */
function runningStatus() {
  return {
    running: true,
    port: 8799,
    url: LAN_URL,
    urls: [LAN_URL, CGNAT_URL],
    fingerprint: null,
    scheme: "https",
  };
}

/** Stopped-service status with the given persisted extras. */
function stoppedStatus(extra: Partial<WebServerStatus>): WebServerStatus {
  return {
    running: false,
    port: null,
    url: null,
    urls: [],
    fingerprint: null,
    autostartError: null,
    savedPort: null,
    autoStart: false,
    scheme: null,
    ...extra,
  };
}

function mockDefaults() {
  webServerStatusMock.mockResolvedValue(runningStatus());
  networkInterfacesListMock.mockResolvedValue([
    { name: "en0", ip: "192.168.1.5", vpn: false },
    { name: "utun3", ip: "100.100.83.2", vpn: true },
  ]);
  webPairingCreateMock.mockImplementation((address?: string) =>
    Promise.resolve({
      url: `${address ? `https://${address}:8799` : LAN_URL}/#pair=tok`,
      deviceToken: "device-token",
    }),
  );
}

/** Mocks for the stopped-panel tests: no interfaces needed, pairing never auto-triggers. */
function mockStopped(extra: Partial<WebServerStatus>) {
  webServerStatusMock.mockResolvedValue(stoppedStatus(extra));
  networkInterfacesListMock.mockResolvedValue([]);
  webPairingCreateMock.mockResolvedValue({ url: "https://x/#pair=abc", deviceToken: "t" });
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  invokeMock.mockImplementation(defaultInvoke);
  for (const key of Object.keys(appSettings)) delete appSettings[key];
});

describe("advertised-IP selector", () => {
  it("lists Automatic plus all interfaces and drives persistence, pairing link, and primary URL", async () => {
    mockDefaults();
    render(<RemoteAccessPanel onClose={vi.fn()} />);

    // The stopped-state selector is replaced by the running-state one once the status arrives, so
    // re-query until the running selector carries all options.
    await waitFor(() => {
      const sel = screen.getByRole("combobox", { name: "remote.ipLabel" }) as HTMLSelectElement;
      expect(sel.options.length).toBe(3);
    });
    const select = screen.getByRole("combobox", {
      name: "remote.ipLabel",
    }) as HTMLSelectElement;
    // Automatic plus both interfaces, with the interface name and the VPN mark visible.
    const labels = Array.from(select.options).map((o) => o.textContent);
    expect(labels[0]).toBe("remote.ipAuto");
    expect(labels).toContain("192.168.1.5 · en0");
    expect(labels).toContain("100.100.83.2 · utun3 (remote.ipVpn)");

    // The automatic pairing on open passes no address.
    await waitFor(() =>
      expect(webPairingCreateMock).toHaveBeenCalledWith(undefined, false),
    );

    fireEvent.change(select, { target: { value: "100.100.83.2" } });

    // Selection is persisted under the share-IP key and regenerates the link with exactly that address.
    expect(invokeMock).toHaveBeenCalledWith("set_app_settings", {
      entries: { "vlx-share-ip": "100.100.83.2" },
    });
    await waitFor(() =>
      expect(webPairingCreateMock).toHaveBeenCalledWith("100.100.83.2", false),
    );

    // The primary displayed/copied pairing URL carries the chosen host.
    await waitFor(() => {
      const copyButtons = screen.getAllByTitle("remote.copyUrl");
      expect(copyButtons[0].textContent).toContain("https://100.100.83.2:8799/#pair=tok");
    });

    // AC 3 explicitly: the QR encodes the pairing URL for the chosen host, not just the copy button.
    await waitFor(() => {
      const qr = document.querySelector("[data-qr-value]");
      expect(qr?.getAttribute("data-qr-value")).toBe("https://100.100.83.2:8799/#pair=tok");
    });
  });

  it("restores a persisted, present IP on open and drives the pairing link with it", async () => {
    mockDefaults();
    appSettings["vlx-share-ip"] = "100.100.83.2";
    render(<RemoteAccessPanel onClose={vi.fn()} />);

    // The restored setting shows in the selector once interfaces and settings have arrived.
    await waitFor(() => {
      const sel = screen.getByRole("combobox", { name: "remote.ipLabel" }) as HTMLSelectElement;
      expect(sel.options.length).toBe(3);
      expect(sel.value).toBe("100.100.83.2");
    });

    // The setting may arrive after the automatic pairing; the regeneration effect must then re-pair
    // with the restored address (timing-sensitive path: pairUrl guard in RemoteAccessPanel).
    await waitFor(() =>
      expect(webPairingCreateMock).toHaveBeenCalledWith("100.100.83.2", false),
    );

    // The primary displayed/copied URL carries the restored host.
    await waitFor(() => {
      const copyButtons = screen.getAllByTitle("remote.copyUrl");
      expect(copyButtons[0].textContent).toContain("https://100.100.83.2:8799/#pair=tok");
    });
  });

  it("falls back to Automatic when the persisted IP is absent, without erasing the stored value", async () => {
    mockDefaults();
    appSettings["vlx-share-ip"] = "10.9.9.9"; // e.g. a VPN interface that is currently down
    render(<RemoteAccessPanel onClose={vi.fn()} />);

    // Wait for the running-state selector with the full option list, then check the shown value.
    await waitFor(() => {
      const sel = screen.getByRole("combobox", { name: "remote.ipLabel" }) as HTMLSelectElement;
      expect(sel.options.length).toBe(3);
      expect(sel.value).toBe("");
    });

    // Pairing uses the automatic backend default; no call ever passes the vanished address.
    await waitFor(() =>
      expect(webPairingCreateMock).toHaveBeenCalledWith(undefined, false),
    );
    expect(webPairingCreateMock).not.toHaveBeenCalledWith("10.9.9.9", false);

    // The stored value is untouched; only an explicit re-selection would overwrite it.
    expect(appSettings["vlx-share-ip"]).toBe("10.9.9.9");
  });

  it("derives primary URL and QR for a selected IP absent from the start-time snapshot", async () => {
    // status.urls mirrors the interface snapshot frozen at server START; the selector enumerates
    // LIVE. Simulate Tailscale connecting after start: 100.101.0.7 is selectable but not in urls.
    webServerStatusMock.mockResolvedValue(runningStatus());
    networkInterfacesListMock.mockResolvedValue([
      { name: "en0", ip: "192.168.1.5", vpn: false },
      { name: "utun4", ip: "100.101.0.7", vpn: true },
    ]);
    // Address-independent pairing mock: the returned URL never echoes the requested address, so the
    // assertions below can only be satisfied by the display derivation (urls → pairUrls), never by a
    // `[pairUrl]` fallback echoing the mock input.
    webPairingCreateMock.mockResolvedValue({
      url: `${LAN_URL}/#pair=tok`,
      deviceToken: "device-token",
    });
    render(<RemoteAccessPanel onClose={vi.fn()} />);

    await waitFor(() => {
      const sel = screen.getByRole("combobox", { name: "remote.ipLabel" }) as HTMLSelectElement;
      expect(sel.options.length).toBe(3);
    });
    fireEvent.change(screen.getByRole("combobox", { name: "remote.ipLabel" }), {
      target: { value: "100.101.0.7" },
    });

    await waitFor(() =>
      expect(webPairingCreateMock).toHaveBeenCalledWith("100.101.0.7", false),
    );

    // Even though 100.101.0.7 is missing from status.urls, the primary displayed/copied pairing
    // URL and the QR carry it — derived from the snapshot's scheme and the live port.
    await waitFor(() => {
      const copyButtons = screen.getAllByTitle("remote.copyUrl");
      expect(copyButtons[0].textContent).toContain("https://100.101.0.7:8799/#pair=tok");
    });
    await waitFor(() => {
      const qr = document.querySelector("[data-qr-value]");
      expect(qr?.getAttribute("data-qr-value")).toBe("https://100.101.0.7:8799/#pair=tok");
    });
  });

  it("re-pairs when the persisted IP arrives while the first pairing is in flight; a stale response never wins", async () => {
    webServerStatusMock.mockResolvedValue(runningStatus());
    networkInterfacesListMock.mockResolvedValue([
      { name: "en0", ip: "192.168.1.5", vpn: false },
      { name: "utun3", ip: "100.100.83.2", vpn: true },
    ]);
    // Controllable pairing requests: each call parks a deferred so the test dictates resolution order.
    const pending: Array<{
      address: string | undefined;
      resolve: (v: { url: string; deviceToken: string }) => void;
    }> = [];
    webPairingCreateMock.mockImplementation(
      (address?: string) =>
        new Promise<{ url: string; deviceToken: string }>((resolve) =>
          pending.push({ address, resolve }),
        ),
    );
    // Defer the persisted settings so they arrive strictly AFTER the automatic pairing started.
    let resolveSettings!: (v: Record<string, string>) => void;
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === "get_app_settings")
        return new Promise<Record<string, string>>((res) => {
          resolveSettings = res;
        });
      return Promise.resolve(null);
    });

    render(<RemoteAccessPanel onClose={vi.fn()} />);

    // The automatic first pairing starts (no address) and stays in flight.
    await waitFor(() => expect(pending.length).toBe(1));
    expect(pending[0].address).toBeUndefined();

    // The persisted IP arrives while that request is pending: a re-pair with it must be issued
    // (the old pairUrl-guarded effect never fired here because pairUrl was still null).
    resolveSettings({ "vlx-share-ip": "100.100.83.2" });
    await waitFor(() => expect(pending.length).toBe(2));
    expect(pending[1].address).toBe("100.100.83.2");

    // The newer request resolves first; its link is displayed.
    pending[1].resolve({ url: "https://100.100.83.2:8799/#pair=tok", deviceToken: "t" });
    await waitFor(() => {
      const copyButtons = screen.getAllByTitle("remote.copyUrl");
      expect(copyButtons[0].textContent).toContain("https://100.100.83.2:8799/#pair=tok");
    });

    // The stale automatic request resolves late and must NOT overwrite the newer link.
    pending[0].resolve({ url: `${LAN_URL}/#pair=stale`, deviceToken: "t" });
    await new Promise((r) => setTimeout(r, 0));
    const copyButtons = screen.getAllByTitle("remote.copyUrl");
    expect(copyButtons[0].textContent).toContain("https://100.100.83.2:8799/#pair=tok");
    expect(copyButtons[0].textContent).not.toContain("stale");
  });
});

describe("stopped-state IP selector", () => {
  it("shows the selector while stopped, and a pre-start selection drives the first pairing after start", async () => {
    webServerStatusMock.mockResolvedValue(stoppedStatus({}));
    networkInterfacesListMock.mockResolvedValue([
      { name: "en0", ip: "192.168.1.5", vpn: false },
      { name: "utun3", ip: "100.100.83.2", vpn: true },
    ]);
    webPairingCreateMock.mockImplementation((address?: string) =>
      Promise.resolve({
        url: `${address ? `https://${address}:8799` : LAN_URL}/#pair=tok`,
        deviceToken: "device-token",
      }),
    );
    webServerStartMock.mockResolvedValue(runningStatus());
    render(<RemoteAccessPanel onClose={vi.fn()} />);

    // The stopped panel shows the selector (next to the port field) with all options.
    await waitFor(() => {
      const sel = screen.getByRole("combobox", { name: "remote.ipLabel" }) as HTMLSelectElement;
      expect(sel.options.length).toBe(3);
    });
    expect(screen.getByPlaceholderText("8799")).toBeTruthy();

    // Select an IP BEFORE starting, then start the service.
    fireEvent.change(screen.getByRole("combobox", { name: "remote.ipLabel" }), {
      target: { value: "100.100.83.2" },
    });
    expect(appSettings["vlx-share-ip"]).toBe("100.100.83.2");
    fireEvent.change(screen.getByPlaceholderText("remote.passwordPlaceholder"), {
      target: { value: "pw" },
    });
    fireEvent.click(screen.getByText("remote.start"));

    // The first pairing after start carries the pre-start selection, never the automatic default.
    await waitFor(() =>
      expect(webPairingCreateMock).toHaveBeenCalledWith("100.100.83.2", false),
    );
    expect(webPairingCreateMock).not.toHaveBeenCalledWith(undefined, false);

    // After a microtask flush still exactly ONE pairing call (with the chosen address): a later
    // automatic default call firing after the positive waitFor would otherwise slip through.
    await new Promise((r) => setTimeout(r, 0));
    expect(webPairingCreateMock).toHaveBeenCalledTimes(1);
  });

  it("stop invalidates in-flight pairing responses; the next start pairs fresh", async () => {
    webServerStatusMock.mockResolvedValue(runningStatus());
    networkInterfacesListMock.mockResolvedValue([
      { name: "en0", ip: "192.168.1.5", vpn: false },
    ]);
    // Controllable pairing requests, parked as deferreds so the test dictates resolution order.
    const pending: Array<{
      address: string | undefined;
      resolve: (v: { url: string; deviceToken: string }) => void;
    }> = [];
    webPairingCreateMock.mockImplementation(
      (address?: string) =>
        new Promise<{ url: string; deviceToken: string }>((resolve) =>
          pending.push({ address, resolve }),
        ),
    );
    render(<RemoteAccessPanel onClose={vi.fn()} />);

    // The automatic pairing after startup is issued and stays in flight.
    await waitFor(() => expect(pending.length).toBe(1));

    // Stop the service while that request is pending.
    webServerStatusMock.mockResolvedValue(stoppedStatus({}));
    fireEvent.click(screen.getByText("remote.stop"));
    await waitFor(() => screen.getByText("remote.start"));

    // The parked response resolves late: it must NOT revive a pairing block in the stopped panel.
    pending[0].resolve({ url: `${LAN_URL}/#pair=stale`, deviceToken: "t" });
    await new Promise((r) => setTimeout(r, 0));
    expect(screen.queryByTitle("remote.copyUrl")).toBeNull();

    // A subsequent start pairs fresh and shows the new link, never the stale one.
    webServerStartMock.mockResolvedValue(runningStatus());
    fireEvent.change(screen.getByPlaceholderText("remote.passwordPlaceholder"), {
      target: { value: "pw" },
    });
    fireEvent.click(screen.getByText("remote.start"));
    await waitFor(() => expect(pending.length).toBe(2));
    // Before the fresh pairing resolves, the running panel must not resurrect the stale link:
    // the stop-time invalidation discarded the in-flight response, so no pairing URL exists yet.
    for (const btn of screen.queryAllByTitle("remote.copyUrl")) {
      expect(btn.textContent).not.toContain("stale");
    }
    pending[1].resolve({ url: `${LAN_URL}/#pair=fresh`, deviceToken: "t" });
    await waitFor(() => {
      const copyButtons = screen.getAllByTitle("remote.copyUrl");
      expect(copyButtons[0].textContent).toContain("#pair=fresh");
      expect(copyButtons[0].textContent).not.toContain("stale");
    });
  });

  it("stop resets pairing even when the follow-up status query fails; the regenerate button is never stuck busy", async () => {
    webServerStatusMock.mockResolvedValue(runningStatus());
    networkInterfacesListMock.mockResolvedValue([
      { name: "en0", ip: "192.168.1.5", vpn: false },
    ]);
    // The automatic pairing request stays in flight for the whole test.
    const pending: Array<{ resolve: (v: { url: string; deviceToken: string }) => void }> = [];
    webPairingCreateMock.mockImplementation(
      () =>
        new Promise<{ url: string; deviceToken: string }>((resolve) =>
          pending.push({ resolve }),
        ),
    );
    render(<RemoteAccessPanel onClose={vi.fn()} />);
    await waitFor(() => expect(pending.length).toBe(1));
    // While the pairing request is in flight, the regenerate button reports busy.
    expect(screen.getByText("remote.pairingCreating")).toBeTruthy();

    // Stop succeeds but the follow-up status query FAILS: the panel keeps showing the running view,
    // so the status-driven effect never fires — the stop path itself must reset the pairing state.
    webServerStatusMock.mockRejectedValue(new Error("status query failed"));
    fireEvent.click(screen.getByText("remote.stop"));
    // Completeness of the reset: pairBusy is cleared although the stranded request's own `finally`
    // skips it (its sequence number was invalidated) — otherwise the button stays disabled forever.
    await waitFor(() => expect(screen.getByText("remote.pairingRegenerate")).toBeTruthy());

    // The stranded response resolves late and must not populate a link for the stopped service.
    pending[0].resolve({ url: `${LAN_URL}/#pair=stale`, deviceToken: "t" });
    await new Promise((r) => setTimeout(r, 0));
    expect(screen.queryByTitle("remote.copyUrl")).toBeNull();
  });
});

describe("urlsForSelectedIp", () => {
  it("reorders like orderUrlsBySelectedIp when the selected IP is in the snapshot", () => {
    expect(urlsForSelectedIp([LAN_URL, CGNAT_URL], "100.100.83.2", 8799, "https")).toEqual([
      CGNAT_URL,
      LAN_URL,
    ]);
  });

  it("synthesizes a URL from the explicit backend scheme and the live port when the IP is missing", () => {
    expect(urlsForSelectedIp([LAN_URL, CGNAT_URL], "100.101.0.7", 8799, "https")).toEqual([
      "https://100.101.0.7:8799",
      LAN_URL,
      CGNAT_URL,
    ]);
    // The plaintext-HTTP mode's scheme is preserved.
    expect(urlsForSelectedIp(["http://192.168.1.5:8080"], "10.0.0.7", 8080, "http")).toEqual([
      "http://10.0.0.7:8080",
      "http://192.168.1.5:8080",
    ]);
    // The explicit backend-reported scheme wins; the snapshot is never consulted when it exists.
    expect(urlsForSelectedIp(["https://192.168.1.5:8080"], "10.0.0.7", 8080, "http")).toEqual([
      "http://10.0.0.7:8080",
      "https://192.168.1.5:8080",
    ]);
  });

  it("falls back to snapshot inference only when the backend reports no scheme", () => {
    // Stale backend without the scheme field: infer from the first snapshot URL, as before.
    expect(urlsForSelectedIp([LAN_URL, CGNAT_URL], "100.101.0.7", 8799, null)).toEqual([
      "https://100.101.0.7:8799",
      LAN_URL,
      CGNAT_URL,
    ]);
    expect(urlsForSelectedIp(["http://192.168.1.5:8080"], "10.0.0.7", 8080, null)).toEqual([
      "http://10.0.0.7:8080",
      "http://192.168.1.5:8080",
    ]);
  });

  it("returns the list unchanged for automatic, an empty snapshot, or an unknown port", () => {
    expect(urlsForSelectedIp([LAN_URL, CGNAT_URL], "", 8799, "https")).toEqual([
      LAN_URL,
      CGNAT_URL,
    ]);
    expect(urlsForSelectedIp([], "10.0.0.7", 8799, "https")).toEqual([]);
    expect(urlsForSelectedIp([LAN_URL], "10.0.0.7", null, "https")).toEqual([LAN_URL]);
  });
});

describe("orderUrlsBySelectedIp", () => {
  it("moves the selected IP's URL to the front and keeps backend order otherwise", () => {
    expect(orderUrlsBySelectedIp([LAN_URL, CGNAT_URL], "100.100.83.2")).toEqual([
      CGNAT_URL,
      LAN_URL,
    ]);
  });

  it("leaves order unchanged for automatic or an absent IP", () => {
    expect(orderUrlsBySelectedIp([LAN_URL, CGNAT_URL], "")).toEqual([LAN_URL, CGNAT_URL]);
    expect(orderUrlsBySelectedIp([LAN_URL, CGNAT_URL], "10.9.9.9")).toEqual([
      LAN_URL,
      CGNAT_URL,
    ]);
  });

  it("matches the host exactly, never by substring", () => {
    const urls = ["https://10.0.0.11:8799", "https://10.0.0.1:8799"];
    expect(orderUrlsBySelectedIp(urls, "10.0.0.1")).toEqual([
      "https://10.0.0.1:8799",
      "https://10.0.0.11:8799",
    ]);
  });
});

describe("RemoteAccessPanel restart persistence", () => {
  it("prefills the port field with the persisted savedPort instead of 8799", async () => {
    mockStopped({ savedPort: 9123 });
    render(<RemoteAccessPanel onClose={() => {}} />);
    await waitFor(() => {
      const input = screen.getByPlaceholderText("8799") as HTMLInputElement;
      expect(input.value).toBe("9123");
    });
  });

  it("keeps the 8799 default when no port was persisted", async () => {
    mockStopped({});
    render(<RemoteAccessPanel onClose={() => {}} />);
    await waitFor(() => {
      const input = screen.getByPlaceholderText("8799") as HTMLInputElement;
      expect(input.value).toBe("8799");
    });
  });

  it("shows the auto-start error reported by the backend", async () => {
    mockStopped({ autostartError: "Port 9123 is already in use" });
    render(<RemoteAccessPanel onClose={() => {}} />);
    await waitFor(() => {
      expect(
        screen.getByText(/remote\.autostartFailed Port 9123 is already in use/),
      ).toBeTruthy();
    });
  });
});
