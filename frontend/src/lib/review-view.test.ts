import { describe, expect, it } from "vitest";
import { baseOptions, reloadsOn } from "./review-view";

describe("the base picker", () => {
  it("offers the reviewed branch and excludes remote HEAD pointers", () => {
    expect(baseOptions(["refs/heads/topic", "refs/heads/feature/HEAD", "refs/remotes/origin/HEAD"], "")).toEqual([
      "refs/heads/topic",
      "refs/heads/feature/HEAD",
    ]);
  });

  it("filters by substring without regard to case", () => {
    expect(baseOptions(["refs/heads/topic", "refs/remotes/origin/release-2"], " LEASE ")).toEqual([
      "refs/remotes/origin/release-2",
    ]);
  });
});

describe("reloading", () => {
  it("follows the refs always and the working tree only while it is included", () => {
    expect(reloadsOn("refs", false)).toBe(true);
    expect(reloadsOn("workingTree", false)).toBe(false);
    expect(reloadsOn("workingTree", true)).toBe(true);
    expect(reloadsOn("stash", true)).toBe(false);
  });
});
