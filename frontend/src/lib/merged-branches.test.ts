import { describe, expect, it } from "vitest";
import { deletedMergedText, marksByName, mergeHint, mergeMark } from "./merged-branches";
import type { BranchMerge } from "$lib/ipc";

const merge = (name: string, oid: string, kind: BranchMerge["kind"]): BranchMerge => ({ name, oid, kind });

describe("mergeMark", () => {
  it("names each way a branch got into HEAD", () => {
    expect(mergeMark("merged")).toBe("merged");
    expect(mergeMark("squash")).toBe("merged by squash");
    expect(mergeMark("rebase")).toBe("merged by rebase");
    expect(mergeMark("notChecked")).toBe("merge not checked");
    expect(mergeMark("notMerged")).toBeUndefined();
  });

  it("says force is needed only where git's plain delete refuses", () => {
    expect(mergeHint("squash")).toContain("needs force");
    expect(mergeHint("rebase")).toContain("needs force");
    expect(mergeHint("merged")).not.toContain("force");
  });
});

describe("marksByName", () => {
  it("keeps an answer only while the branch is still at the tip it was judged on", () => {
    const tips = new Map([
      ["a", "1"],
      ["b", "9"],
    ]);
    const marks = marksByName([merge("a", "1", "squash"), merge("b", "2", "rebase"), merge("gone", "3", "merged")], tips);
    expect([...marks]).toEqual([["a", "squash"]]);
  });
});

describe("deletedMergedText", () => {
  it("is nothing when nothing was deleted", () => {
    expect(deletedMergedText([])).toBeNull();
  });

  it("says how each branch was merged unless it was plainly", () => {
    expect(deletedMergedText([merge("old", "1", "merged"), merge("topic", "2", "squash"), merge("fix", "3", "rebase")])).toBe(
      "Deleted merged branches: old, topic (merged by squash), fix (merged by rebase)",
    );
    expect(deletedMergedText([merge("topic", "2", "squash")])).toBe("Deleted merged branch: topic (merged by squash)");
  });
});
