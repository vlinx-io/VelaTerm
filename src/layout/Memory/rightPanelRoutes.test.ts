import { expect, it } from "vitest";
import { memoryRouteHidesRightPanel } from "./navigation";

it("keeps the right panel for entries, revisions and archived sessions only", () => {
  expect(memoryRouteHidesRightPanel("?memory=notebooks")).toBe(true);
  expect(memoryRouteHidesRightPanel("?memory=jobs")).toBe(true);
  expect(memoryRouteHidesRightPanel("?memory=collections")).toBe(true);
  expect(memoryRouteHidesRightPanel("?memory=entry%2Fe1")).toBe(false);
  expect(memoryRouteHidesRightPanel("?memory=history%2Fe1%2F2")).toBe(false);
  expect(memoryRouteHidesRightPanel("?memory=collection%2Fs1")).toBe(false);
  expect(memoryRouteHidesRightPanel("")).toBe(false);
});
