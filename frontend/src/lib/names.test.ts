import { describe, expect, it } from "vitest";
import { branchNameProblem, typedRefName, optional, presetId, presetNameProblem, promptProblem, textProblem } from "./names";

describe("a preset's name", () => {
  it("makes the id from its Latin letters and digits", () => {
    expect(presetId("  My Lint: v2 ")).toBe("my-lint-v2");
  });

  it("is refused in the dialog when it has none of them", () => {
    expect(presetNameProblem("Проверка")).toContain("Latin letters or digits");
    expect(presetNameProblem("Проверка 2")).toBeNull();
    expect(presetNameProblem(" ")).toBe("Enter a value.");
  });
});

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

describe("typedRefName", () => {
  it("types an underscore for a space, the caret where it was", () => {
    expect(typedRefName("fix login", 9, 9)).toEqual({ value: "fix_login", start: 9, end: 9 });
    expect(typedRefName("a b", 2, 2)).toEqual({ value: "a_b", start: 2, end: 2 });
  });

  it("turns every space of a paste, and leaves a name without spaces as it is", () => {
    expect(typedRefName("new feature x", 13, 13)?.value).toBe("new_feature_x");
    expect(typedRefName("feature/x", 3, 5)).toBeNull();
  });
});
