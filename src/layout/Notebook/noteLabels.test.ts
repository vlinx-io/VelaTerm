import { describe, expect, it } from "vitest";
import { homePath, noteFolder, noteTitle } from "./shared";

describe("note labels", () => {
  it("drops the extension and a trailing file name timestamp from titles", () => {
    expect(noteTitle("系统负载_20260904_1346.md")).toBe("系统负载");
    expect(noteTitle("Plan_20260904.md")).toBe("Plan");
    expect(noteTitle("Release 20260904.md")).toBe("Release");
    expect(noteTitle("Notes 2026.md")).toBe("Notes 2026");
    expect(noteTitle("_20260904_1346.md")).toBe("_20260904_1346");
  });

  it("names a note's folder and leaves the root empty", () => {
    expect(noteFolder("技术研究/AI/note.md")).toBe("技术研究/AI");
    expect(noteFolder("note.md")).toBe("");
  });

  it("shortens only paths inside the home directory", () => {
    expect(homePath("/Users/a/Notes", "/Users/a")).toBe("~/Notes");
    expect(homePath("/Users/a", "/Users/a/")).toBe("~");
    expect(homePath("/Users/ab/Notes", "/Users/a")).toBe("/Users/ab/Notes");
    expect(homePath("C:\\Users\\a\\Notes", "C:\\Users\\a")).toBe("~\\Notes");
    expect(homePath("/Users/a/Notes", null)).toBe("/Users/a/Notes");
  });
});
