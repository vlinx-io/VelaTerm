import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

vi.mock("../../../platform", () => ({ platform: { opener: { openExternal: vi.fn() } } }));
vi.mock("../../../store/termStore", () => ({ useTermStore: { getState: () => ({ openDocTab }) } }));
const { openDocTab } = vi.hoisted(() => ({ openDocTab: vi.fn() }));

import { platform } from "../../../platform";
import { Markdown } from "./markdown";
import { resolveSessionLink, SessionLinkDirectory } from "./links";

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(platform.opener.openExternal).mockResolvedValue(undefined);
});
afterEach(cleanup);

it("opens the screenshot link in an application document tab instead of navigating to a web path", () => {
  render(<Markdown text="[查看对照截图](/tmp/vlx-chinese-font-comparison.png)" />);
  const link = screen.getByRole("link", { name: "查看对照截图" });
  expect(link.getAttribute("href")).toBe("/tmp/vlx-chinese-font-comparison.png");
  expect(fireEvent.click(link)).toBe(false);
  expect(openDocTab).toHaveBeenCalledWith("/tmp/vlx-chinese-font-comparison.png");
  expect(platform.opener.openExternal).not.toHaveBeenCalled();
});

it("resolves a relative file using its containing session's directory", () => {
  render(<SessionLinkDirectory.Provider value="/work/project">
    <Markdown text="[设计文档](<docs/中文 design.md>)" />
  </SessionLinkDirectory.Provider>);
  fireEvent.click(screen.getByRole("link"));
  expect(openDocTab).toHaveBeenCalledWith("/work/project/docs/中文 design.md");
});

it.each([
  ["file:///tmp/hello%20world.png", undefined, "/tmp/hello world.png"],
  ["/work/app.ts:12:3", undefined, "/work/app.ts"],
  ["../README.md#L12-L15", "/work/src", "/work/README.md"],
  ["C:\\work\\中文.png", undefined, "C:/work/中文.png"],
  ["file:///C:/work/a.png", undefined, "C:/work/a.png"],
  ["docs/a.md", "C:\\work", "C:/work/docs/a.md"],
])("resolves the local reference %s", (href, cwd, path) => {
  expect(resolveSessionLink(href, cwd)).toEqual({ kind: "file", path });
});

it("uses the platform opener for web links, including clicks on formatted labels", () => {
  render(<Markdown text="[**官网**](https://example.com/docs?a=1#intro)" />);
  fireEvent.click(screen.getByText("官网"));
  expect(platform.opener.openExternal).toHaveBeenCalledWith("https://example.com/docs?a=1#intro");
  expect(openDocTab).not.toHaveBeenCalled();
});

it("reports an opener failure in the message", async () => {
  vi.mocked(platform.opener.openExternal).mockRejectedValue(new Error("Open failed"));
  render(<Markdown text="[官网](https://example.com)" />);
  fireEvent.click(screen.getByRole("link"));
  expect((await screen.findByRole("alert")).textContent).toContain("Open failed");
});

it("routes a middle click on a file to the application viewer", () => {
  render(<Markdown text="[截图](/tmp/a.png)" />);
  fireEvent(screen.getByRole("link"), new MouseEvent("auxclick", { bubbles: true, cancelable: true, button: 1 }));
  expect(openDocTab).toHaveBeenCalledTimes(1);
});

it.each(["javascript:alert(1)", "data:text/html,test", "custom:command", "/tmp/a%00.png"])(
  "does not activate unsafe link %s", (href) => {
    expect(resolveSessionLink(href, "/work")).toBeNull();
  },
);
