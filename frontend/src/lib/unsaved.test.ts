import { describe, expect, it } from "vitest";
import { unsavedSummary } from "./unsaved";

describe("unsavedSummary", () => {
  it("is null when nothing is open", () => {
    expect(unsavedSummary({ hook: null, merge: null })).toBeNull();
  });

  it("names an edited hook", () => {
    expect(unsavedSummary({ hook: "pre-commit", merge: null })).toContain("pre-commit");
  });

  it("names a resolution in progress", () => {
    expect(unsavedSummary({ hook: null, merge: "a.txt" })).toContain("a.txt");
  });

  it("names both when both are open", () => {
    const text = unsavedSummary({ hook: "pre-push", merge: "b.txt" });
    expect(text).toContain("pre-push");
    expect(text).toContain("b.txt");
  });

  it("agrees with itself about number", () => {
    expect(unsavedSummary({ hook: "pre-commit", merge: null })).toContain(" has ");
    expect(unsavedSummary({ hook: "pre-commit", merge: "b.txt" })).toContain(" have ");
  });
});
