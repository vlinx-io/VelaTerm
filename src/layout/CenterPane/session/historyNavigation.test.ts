import { afterEach, expect, it } from "vitest";
import { historyLocation, historyPageURL } from "./historyNavigation";

afterEach(() => window.history.replaceState(null, "", "/"));

it("copies exact paging locations without carrying a field from the previous row", () => {
  window.history.replaceState(null, "", "/?chatRow=old&chatField=old-ref&chatPart=8&chatSearch=needle");
  const older = historyPageURL("session", "before", "stable/row");
  window.history.pushState(null, "", older);
  expect(historyLocation("session")).toEqual({ before: "stable/row", after: undefined, anchor: undefined });
  expect(historyLocation("other-session")).toEqual({});
  expect(new URL(older).searchParams.get("chatField")).toBeNull();
  expect(new URL(older).searchParams.get("chatSearch")).toBe("needle");
  window.history.replaceState(null, "", historyPageURL("session", "after", "newer-row"));
  expect(historyLocation("session")).toEqual({ before: undefined, after: "newer-row", anchor: undefined });
});

it("locates the first parent before opening a copied nested child detail",()=>{
  window.history.replaceState(null,"","/?chatSession=session&chatChild=parent&chatChild=nested&chatRow=answer");
  expect(historyLocation("session").anchor).toBe("parent");
  expect(new URL(historyPageURL("session","after","newer")).searchParams.has("chatChild")).toBe(false);
});

it("ignores locations that do not name their conversation", () => {
  // A child path left behind after leaving its conversation must not locate rows in a new one.
  window.history.replaceState(null, "", "/?session=new&chatChild=parent-row&chatRow=answer&chatBefore=older");
  expect(historyLocation("new")).toEqual({});
});
