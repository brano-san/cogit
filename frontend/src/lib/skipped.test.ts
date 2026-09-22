import { describe, expect, it } from "vitest";
import { describeSkipped } from "./skipped";

describe("describeSkipped", () => {
  it("says nothing when every ticked ref was drawn", () => {
    expect(describeSkipped([])).toBeNull();
  });

  it("names the ref by its short name and says why", () => {
    expect(
      describeSkipped([{ name: "refs/tags/v-tree", reason: "Tag does not point to a commit" }]),
    ).toBe("Not in the graph: v-tree — Tag does not point to a commit.");
  });

  it("lists several, each with its reason", () => {
    const text = describeSkipped([
      { name: "refs/tags/a", reason: "Tag does not point to a commit" },
      { name: "refs/heads/b", reason: "Tag does not point to a commit" },
    ]);
    expect(text).toContain("a — ");
    expect(text).toContain("b — ");
  });
});
