import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
vi.mock("../../../ipc/transport", () => ({ invoke: vi.fn() }));
import { invoke } from "../../../ipc/transport";
import { ZcodeStartPlanStatus } from "./ZcodeStartPlanStatus";
afterEach(cleanup);
it("空白会话展示套餐待生效时间，刷新后展示官方原始额度", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({ plans: [{ providerId: "account:bigmodel-start-plan", label: "BigModel · Start Plan",
    status: "pending", effectiveAt: 1792076400, models: [], balances: [],
    plans: [{ name: "ZCode Global Build", status: "active", effectiveAt: 1792076400 }],
  }] }).mockResolvedValueOnce({ plans: [{ providerId: "account:bigmodel-start-plan", label: "BigModel · Start Plan",
    status: "available", models: ["GLM-5.3"], plans: [],
    balances: [{ name: "GLM-5.3", unit: "credits", total: 100, used: 20, available: 80, models: ["GLM-5.3"] }],
  }] });
  const refreshed = vi.fn();
  render(<ZcodeStartPlanStatus sessionId="cold" onRefreshed={refreshed} />);
  expect(await screen.findByText("ZCode Global Build")).toBeTruthy();
  expect(screen.getByText(/待生效|Pending/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button"));
  expect(await screen.findByText(/80/)).toBeTruthy();
  expect(screen.getByText(/credits/)).toBeTruthy();
  expect(refreshed).toHaveBeenCalled();
  expect(invoke).toHaveBeenCalledWith("chat_zcode_start_plan", { sessionId: "cold" });
});
