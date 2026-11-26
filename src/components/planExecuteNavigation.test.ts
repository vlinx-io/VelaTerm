import { afterEach, describe, expect, it, vi } from "vitest";
import {
  bindPlanExecuteUrl,
  navigateBindPlanExecute,
  planExecuteUrl,
} from "./planExecuteNavigation";

function setLocation(href: string) {
  Object.defineProperty(window, "location", {
    configurable: true,
    value: new URL(href),
  });
}

describe("planExecuteNavigation bind URL handling", () => {
  const originalLocation = window.location;

  afterEach(() => {
    Object.defineProperty(window, "location", {
      configurable: true,
      value: originalLocation,
    });
    vi.restoreAllMocks();
  });

  it("constructs and clears bindPlanExecute query params on http localhost", () => {
    setLocation("http://localhost:18733/?foo=1&planExecute=old&planProject=p1");
    const built = bindPlanExecuteUrl({
      requestId: "req-1",
      plannerId: "s-plan",
      executorIds: ["s-exec-1", "s-exec-2"],
    });
    const parsed = new URL(built);
    expect(parsed.origin).toBe("http://localhost:18733");
    expect(parsed.searchParams.get("foo")).toBe("1");
    expect(parsed.searchParams.get("bindPlanExecute")).toBe("req-1");
    expect(parsed.searchParams.get("bindPlan")).toBe("s-plan");
    expect(parsed.searchParams.get("bindExec")).toBe("s-exec-1,s-exec-2");
    // Mutually exclusive creation route parameters must be cleared.
    expect(parsed.searchParams.has("planExecute")).toBe(false);
    expect(parsed.searchParams.has("planProject")).toBe(false);

    setLocation(built);
    const cleared = bindPlanExecuteUrl(null);
    expect(cleared).toBe("http://localhost:18733/?foo=1");
  });

  it("returns full tauri://localhost URL without empty string when clearing on desktop WebView", () => {
    setLocation("tauri://localhost?bindPlanExecute=req-2&bindRole=exec&bindPlan=s-plan&bindExec=s-e1");
    const cleared = bindPlanExecuteUrl(null);
    expect(cleared).toBe("tauri://localhost");
    expect(cleared).not.toBe("");

    setLocation("tauri://localhost");
    const multi = bindPlanExecuteUrl({
      requestId: "req-3",
      role: "exec",
      sessionIds: ["s-a", "s-b"],
    });
    expect(multi).toBe("tauri://localhost?bindPlanExecute=req-3&bindRole=exec&bindExec=s-a%2Cs-b");

    setLocation(multi);
    // Opening create-new menu clears bindPlanExecute and bindRole parameters.
    const menuHref = planExecuteUrl({ projectId: "p1", groupId: null, parentSessionId: null });
    const menuParsed = new URL(menuHref);
    expect(menuParsed.searchParams.has("bindPlanExecute")).toBe(false);
    expect(menuParsed.searchParams.has("bindRole")).toBe(false);
    expect(menuParsed.searchParams.has("bindExec")).toBe(false);
    expect(menuParsed.searchParams.get("planProject")).toBe("p1");
  });

  it("pushes full url.href and dispatches popstate", () => {
    const pushSpy = vi.spyOn(window.history, "pushState").mockImplementation(() => {});
    const onPop = vi.fn();
    window.addEventListener("popstate", onPop);
    navigateBindPlanExecute("tauri://localhost");
    window.removeEventListener("popstate", onPop);
    expect(pushSpy).toHaveBeenCalledWith(null, "", "tauri://localhost");
    expect(onPop).toHaveBeenCalledTimes(1);
  });
});
