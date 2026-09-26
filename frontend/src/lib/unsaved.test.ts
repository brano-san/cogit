import { describe, expect, it } from "vitest";
import { closeAnswer, unsavedSummary } from "./unsaved";

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

// Edit Git Config: a click beside the window threw the typed text away without a word.
describe("unsaved dialogs", () => {
  it("names a dialog holding typed work", () => {
    expect(unsavedSummary({ hook: null, merge: null, dialogs: ["Edit Git Config — User"] })).toBe(
      "“Edit Git Config — User” has unsaved changes.",
    );
  });

  it("names every one of them with the rest", () => {
    const text = unsavedSummary({ hook: "pre-push", merge: null, dialogs: ["Index Editor — a.txt"] });
    expect(text).toContain("pre-push");
    expect(text).toContain("Index Editor — a.txt");
    expect(text).toContain(" have ");
  });
});

describe("closeAnswer", () => {
  it("closes a dialog with nothing typed, however it is asked", () => {
    for (const request of ["scrim", "escape", "button"] as const) {
      expect(closeAnswer(request, false)).toBe("close");
    }
  });

  it("stays open on a click beside a dialog holding typed work", () => {
    expect(closeAnswer("scrim", true)).toBe("stay");
  });

  it("asks before Esc or the close button throws typed work away", () => {
    expect(closeAnswer("escape", true)).toBe("ask");
    expect(closeAnswer("button", true)).toBe("ask");
  });
});
