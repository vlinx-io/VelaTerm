import { beforeEach, describe, expect, it } from "vitest";

import { useTermStore } from "../store/termStore";
import { sanitizePrefill, takePrefill } from "./prefill";

beforeEach(() => useTermStore.setState({ runtimes: {} }));

describe("text typed into the terminal view", () => {
  it("never carries a control character that could submit or edit the prompt", () => {
    const hostile = "/memory\r\n\x1b[A\t\x03\x7f\u009b2J\u0000x";
    const clean = sanitizePrefill(hostile);
    expect(clean).not.toMatch(/[\u0000-\u001f\u007f-\u009f]/);
    expect(clean.startsWith("/memory")).toBe(true);
    expect(sanitizePrefill("  /status  ")).toBe("/status");
  });

  it("is taken once and then gone", () => {
    useTermStore.getState().setRuntime("s", { agentPrefill: "/hooks\r" });
    expect(takePrefill("s")).toBe("/hooks");
    expect(useTermStore.getState().runtimes.s?.agentPrefill).toBeUndefined();
    expect(takePrefill("s")).toBeUndefined();
  });
});
