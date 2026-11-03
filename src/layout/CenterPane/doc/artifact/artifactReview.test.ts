//! Review messages from the page are validated before they reach state, and comments and option choices become
//! messages that name the element precisely enough for the agent to find it.

import { describe, expect, it } from "vitest";

import { BRIDGE_SOURCE, parseBridgeMessage } from "./artifactBridge";
import {
  addComment, artifactReview, clearComments, commentReport, optionReport, removeComment, reviewMarks, setDraft, setPicking,
} from "./artifactReview";

const msg = (m: Record<string, unknown>) => parseBridgeMessage({ source: BRIDGE_SOURCE, ...m });

describe("review bridge messages", () => {
  it("accepts a picked element and clips its fields", () => {
    expect(msg({ type: "picked", selector: "main > button", tag: "button", text: "x".repeat(500), option: "1a" }))
      .toEqual({ type: "picked", selector: "main > button", tag: "button", text: "x".repeat(120), option: "1a" });
    expect(msg({ type: "picked", selector: "p", tag: "p", text: "", option: null })).toMatchObject({ option: null });
  });

  it("rejects a pick without a selector or with the wrong source", () => {
    expect(msg({ type: "picked", selector: "", tag: "p", text: "" })).toBeNull();
    expect(msg({ type: "picked", tag: "p", text: "" })).toBeNull();
    expect(parseBridgeMessage({ source: "page", type: "picked", selector: "p", tag: "p", text: "" })).toBeNull();
  });

  it("keeps well-formed, distinct options and drops the rest", () => {
    expect(msg({ type: "options", options: [{ id: "1a", label: "Tiles" }, { id: "1a", label: "Again" }, { id: "", label: "x" },
      { id: 3, label: "bad" }, null, { id: "1b", label: "Bar" }] }))
      .toEqual({ type: "options", options: [{ id: "1a", label: "Tiles" }, { id: "1b", label: "Bar" }] });
    expect(msg({ type: "options", options: "1a" })).toBeNull();
  });
});

describe("review draft", () => {
  const path = "/tmp/review-test.html";
  const target = { selector: "#save", tag: "button", text: "Save", option: "1b" };

  it("a pick ends pick mode, and a comment needs text", () => {
    setPicking(path, true);
    setDraft(path, target);
    expect(artifactReview(path)).toMatchObject({ picking: false, draft: target });
    addComment(path, "   ");
    expect(artifactReview(path).comments).toHaveLength(0);
    addComment(path, " Make it green ");
    const review = artifactReview(path);
    expect(review.draft).toBeNull();
    expect(review.comments).toMatchObject([{ selector: "#save", note: "Make it green" }]);
    expect(reviewMarks(review)).toEqual([{ n: 1, selector: "#save" }]);
    removeComment(path, review.comments[0].id);
    expect(artifactReview(path).comments).toHaveLength(0);
    clearComments(path);
  });

  it("marks the element being commented on with pin 0", () => {
    setDraft(path, target);
    expect(reviewMarks(artifactReview(path))).toEqual([{ n: 0, selector: "#save" }]);
    clearComments(path);
  });
});

describe("messages for the agent", () => {
  it("lists each comment with its element, selector and option", () => {
    const text = commentReport("/p/page.html", [
      { id: 1, selector: "#save", tag: "button", text: "Save", option: "1b", note: "Make it green" },
      { id: 2, selector: "h1", tag: "h1", text: "", option: null, note: "Shorter" },
    ]);
    expect(text).toContain("/p/page.html");
    expect(text).toContain("1. <button> “Save” (selector `#save`, in option 1b): Make it green");
    expect(text).toContain("2. <h1> (selector `h1`): Shorter");
  });

  it("names the chosen option and asks to drop the others", () => {
    expect(optionReport("/p/page.html", "1b", "Tiles")).toContain("go with option 1b (Tiles)");
    expect(optionReport("/p/page.html", "2a", "")).toContain("go with option 2a.");
  });
});
