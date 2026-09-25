import { describe, expect, it } from "vitest";
import { branchNameProblem, optional, promptProblem, textProblem } from "./names";

describe("branchNameProblem", () => {
  it("refuses what git refuses and a name already taken", () => {
    expect(branchNameProblem("my branch", [])).not.toBeNull();
    expect(branchNameProblem("main", ["main"])).not.toBeNull();
    expect(branchNameProblem("-x", [])).not.toBeNull();
    expect(branchNameProblem("feature/login", ["main"])).toBeNull();
  });
});

// A prompt that named no rule was checked as a branch name: a group called "Work stuff"
// and a preset called "My check" were refused, and a release could not be finished
// without a tag although the field says "or empty for none".
describe("rules for what is not a branch", () => {
  it("lets any name through that is not empty", () => {
    expect(textProblem("Work stuff")).toBeNull();
    expect(textProblem("  ")).not.toBeNull();
  });

  it("lets an optional field stay empty", () => {
    expect(optional()).toBeNull();
  });
});

describe("promptProblem", () => {
  // `validate?.(text) ?? emptyCheck`: optional() answers null, and `??` fell through to
  // "Enter a value." — a release could not be finished without a tag.
  it("lets the validator accept an empty value", () => {
    expect(promptProblem("", optional)).toBeNull();
  });

  it("goes by the validator's answer when it has one", () => {
    expect(promptProblem("main", (value) => (value === "main" ? "main already exists." : null))).toBe(
      "main already exists.",
    );
  });

  it("asks for something when there is no validator", () => {
    expect(promptProblem("  ")).toBe("Enter a value.");
    expect(promptProblem("x")).toBeNull();
  });
});
