import { describe, expect, it } from "vitest";
import { SUBJECT_HARD, SUBJECT_SOFT, subjectOf, subjectState } from "./commit-message";

describe("subjectOf", () => {
  it("takes the first line", () => {
    expect(subjectOf("summary\n\nbody")).toBe("summary");
  });

  it("returns the whole text when there is only one line", () => {
    expect(subjectOf("summary")).toBe("summary");
  });

  it("counts an empty message as an empty subject", () => {
    expect(subjectOf("")).toBe("");
  });
});

describe("subjectState", () => {
  it("is fine well under the soft limit", () => {
    expect(subjectState("short")).toBe("ok");
  });

  it("is fine exactly at the soft limit", () => {
    expect(subjectState("x".repeat(SUBJECT_SOFT))).toBe("ok");
  });

  it("warns one character past the soft limit", () => {
    expect(subjectState("x".repeat(SUBJECT_SOFT + 1))).toBe("long");
  });

  it("is fine exactly at the hard limit", () => {
    expect(subjectState("x".repeat(SUBJECT_HARD))).toBe("long");
  });

  it("is too long one character past the hard limit", () => {
    expect(subjectState("x".repeat(SUBJECT_HARD + 1))).toBe("too-long");
  });

  it("measures the subject, not the body", () => {
    expect(subjectState(`short\n\n${"x".repeat(200)}`)).toBe("ok");
  });

  it("counts characters, not UTF-16 units, so emoji do not trip it early", () => {
    expect(subjectState("🙂".repeat(30))).toBe("ok");
  });
});
