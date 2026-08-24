//! Covers the bound on `flushNow`. `invoke` has no timeout of its own, and over WebSocket transport a half-open
//! socket settles a request neither with a reply nor with a close, so an unbounded flush can hang forever. The
//! quit dialog awaits this call before approving the exit, which is why the bound has to hold.

import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("./transport", () => ({ invoke }));

import { flushNow, pushSetting, reconcileSettings } from "./settingsSync";

/** Enable propagation the way startup does; `pushSetting` is gated on reconciliation having run. */
async function enableSync() {
  invoke.mockResolvedValueOnce({});
  await reconcileSettings();
}

beforeEach(() => {
  invoke.mockReset();
  localStorage.clear();
});

describe("flushNow", () => {
  it("resolves after the timeout when the backend never answers", async () => {
    await enableSync();
    invoke.mockReturnValueOnce(new Promise(() => {})); // never settles: the half-open-socket case
    pushSetting("vlx-theme", "dark");

    const started = Date.now();
    await flushNow(50);
    expect(Date.now() - started).toBeLessThan(1000); // returned on the timeout, not on the write
  });

  it("still awaits the write when no timeout is given", async () => {
    await enableSync();
    let landed = false;
    invoke.mockImplementationOnce(async () => {
      await new Promise((r) => setTimeout(r, 10));
      landed = true;
    });
    pushSetting("vlx-theme", "dark");

    await flushNow();
    expect(landed).toBe(true);
  });

  it("does not wait at all when nothing is pending", async () => {
    await enableSync();
    await flushNow(50);
    expect(invoke).toHaveBeenCalledTimes(1); // only reconcileSettings' own read
  });
});
