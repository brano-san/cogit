import { describe, expect, it } from "vitest";
import { branchNameProblem, optional, textProblem } from "./names";

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
