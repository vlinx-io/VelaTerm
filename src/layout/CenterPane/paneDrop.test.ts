import { describe, expect, it } from "vitest";
import { dropZone, zonePreview, zoneSplit } from "./paneDrop";

describe("dropZone", () => {
  it("treats the middle of a pane as a replacement", () => {
    expect(dropZone(0.5, 0.5)).toBe("center");
    expect(dropZone(0.3, 0.7)).toBe("center");
  });

  it("picks the nearest edge inside the edge band", () => {
    expect(dropZone(0.05, 0.5)).toBe("left");
    expect(dropZone(0.95, 0.5)).toBe("right");
    expect(dropZone(0.5, 0.1)).toBe("top");
    expect(dropZone(0.5, 0.9)).toBe("bottom");
    // A corner goes to whichever edge is closer.
    expect(dropZone(0.02, 0.1)).toBe("left");
    expect(dropZone(0.2, 0.97)).toBe("bottom");
  });
});

describe("zoneSplit and zonePreview", () => {
  const rect = { left: 50, top: 0, width: 50, height: 40 };

  it("maps edges to split direction and side", () => {
    expect(zoneSplit("left")).toEqual({ dir: "horizontal", before: true });
    expect(zoneSplit("right")).toEqual({ dir: "horizontal", before: false });
    expect(zoneSplit("top")).toEqual({ dir: "vertical", before: true });
    expect(zoneSplit("bottom")).toEqual({ dir: "vertical", before: false });
    expect(zoneSplit("center")).toBeNull();
  });

  it("previews the half a split would occupy, or the whole pane", () => {
    expect(zonePreview(rect, "right")).toEqual({ left: 75, top: 0, width: 25, height: 40 });
    expect(zonePreview(rect, "bottom")).toEqual({ left: 50, top: 20, width: 50, height: 20 });
    expect(zonePreview(rect, "center")).toEqual(rect);
  });
});
