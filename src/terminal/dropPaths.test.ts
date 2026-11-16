import { describe, expect, it } from "vitest";
import { chatDropText, pathsExcept, shellQuotePath, terminalDropText } from "./dropPaths";

describe("dropped path text", () => {
  it("escapes shell characters on Unix and keeps CJK names readable", () => {
    expect(shellQuotePath("/Users/me/My Docs/a (1).txt", false)).toBe("/Users/me/My\\ Docs/a\\ \\(1\\).txt");
    expect(shellQuotePath("/Users/me/项目/说明.md", false)).toBe("/Users/me/项目/说明.md");
    expect(shellQuotePath("/tmp/it's$x&y", false)).toBe("/tmp/it\\'s\\$x\\&y");
  });

  it("double-quotes Windows paths only when needed", () => {
    expect(shellQuotePath("C:\\work\\a.txt", true)).toBe("C:\\work\\a.txt");
    expect(shellQuotePath("C:\\My Docs\\a.txt", true)).toBe('"C:\\My Docs\\a.txt"');
  });

  it("joins terminal paths with a trailing space", () => {
    expect(terminalDropText(["/a b", "/c"], false)).toBe("/a\\ b /c ");
  });

  it("quotes chat paths only around whitespace", () => {
    expect(chatDropText(["/a b/c.txt", "/d.txt"])).toBe('"/a b/c.txt" /d.txt');
  });

  it("leaves out paths of files handled elsewhere", () => {
    const img = new File([""], "shot.png", { type: "image/png" });
    expect(pathsExcept(["/x/shot.png", "/x/notes.md"], [img])).toEqual(["/x/notes.md"]);
  });
});
