import { describe, expect, it } from "vitest";
import { filesPanelList, type FilesPanelInput } from "./files-panel";

const base: FilesPanelInput = {
  content: true,
  stash: null,
  compare: null,
  onWorkingTree: true,
  worktree: 5,
  commit: 0,
};

// The header counted the working tree while the body listed a stash, and the selected
// commit's files while it listed a comparison — one panel, two sources.
describe("what the Files panel lists and counts", () => {
  it("counts a stash's files, not the dirty working tree behind it", () => {
    const stash = { worktree: [1, 2], index: [], untracked: [] };
    expect(filesPanelList({ ...base, stash })).toEqual({ kind: "stash", count: 2 });
  });

  it("counts the comparison, not the selected commit", () => {
    expect(filesPanelList({ ...base, onWorkingTree: false, commit: 7, compare: [1, 2, 3] })).toEqual({
      kind: "compare",
      count: 3,
    });
  });

  it("counts the rows of the commit's list, the unchanged ones included", () => {
    expect(filesPanelList({ ...base, onWorkingTree: false, commit: 40 })).toEqual({ kind: "commit", count: 40 });
  });

  it("counts the working tree on the working tree", () => {
    expect(filesPanelList(base)).toEqual({ kind: "worktree", count: 5 });
  });

  it("counts nothing while no repository is open", () => {
    expect(filesPanelList({ ...base, content: false })).toEqual({ kind: "none", count: undefined });
  });
});
