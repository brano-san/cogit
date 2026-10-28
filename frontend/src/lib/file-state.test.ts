import { describe, expect, it } from "vitest";
import { fileState, sideOfRow, type StateInput } from "./file-state";

const f = (over: Partial<StateInput>): StateInput => ({ status: "modified", ...over });

describe("fileState text by side", () => {
  it.each([
    [f({ status: "untracked" }), "worktree", "Untracked"],
    [f({ status: "modified" }), "worktree", "Modified"],
    [f({ status: "modified" }), "index", "Staged"],
    [f({ status: "modified" }), "commit", "Modified"],
    [f({ status: "added" }), "worktree", "Added"],
    [f({ status: "added" }), "index", "Added"],
    [f({ status: "deleted" }), "worktree", "Missing"],
    [f({ status: "deleted" }), "index", "Removed"],
    [f({ status: "deleted" }), "commit", "Removed"],
    [f({ status: "renamed" }), "index", "Renamed"],
    [f({ status: "copied" }), "index", "Copied"],
    [f({ status: "ignored" }), "worktree", "Ignored"],
    [f({ status: "unchanged" }), "worktree", "Unchanged"],
    [f({ status: "assumeUnchanged" }), "worktree", "Assume unchanged"],
    [f({ status: "skipped" }), "worktree", "Skipped"],
    [f({ status: "sparse" }), "worktree", "Outside sparse checkout"],
    [f({ status: "modified", modeChange: "symlink" }), "index", "Type changed"],
    [f({ status: "modified", modeChange: "executable" }), "worktree", "Modified"],
  ] as const)("%j on %s is %s", (file, side, text) => {
    const state = fileState(file, side);
    expect(state.text).toBe(text);
    expect(state.tone).toBe("secondary");
  });

  it.each([
    ["bothModified", "Conflicted (both modified)"],
    ["bothAdded", "Conflicted (both added)"],
    ["bothDeleted", "Conflicted (both deleted)"],
    ["deletedByThem", "Conflicted (deleted by them)"],
    ["deletedByUs", "Conflicted (deleted by us)"],
    ["addedByUs", "Conflicted (added by us)"],
    ["addedByThem", "Conflicted (added by them)"],
  ] as const)("a %s conflict reads %s in the danger tone", (conflict, text) => {
    const state = fileState(f({ status: "conflicted", conflict }), "worktree");
    expect(state).toMatchObject({ text, tone: "danger", icon: "conflicted" });
  });

  it("a conflict without a kind is just Conflicted", () => {
    expect(fileState(f({ status: "conflicted" }), "worktree").text).toBe("Conflicted");
  });

  it.each([
    { newCommits: true, modified: false, untracked: false },
    { newCommits: true, modified: true, untracked: true },
  ])("a moved submodule %j reads just Modified", (submodule) => {
    const state = fileState(f({ mode: "submodule", submodule }), "worktree");
    expect(state).toMatchObject({ text: "Modified", icon: "modified" });
  });

  it("a staged submodule pointer is Staged", () => {
    expect(fileState(f({ mode: "submodule" }), "index").text).toBe("Staged");
  });
});

describe("fileState icon", () => {
  it.each([
    ["untracked", "worktree", "untracked"],
    ["added", "index", "added"],
    ["modified", "worktree", "modified"],
    ["modified", "index", "staged"],
    ["deleted", "index", "removed"],
    ["deleted", "worktree", "missing"],
    ["renamed", "index", "renamed"],
    ["copied", "index", "renamed"],
    ["ignored", "worktree", "ignored"],
    ["unchanged", "worktree", "unchanged"],
    ["conflicted", "worktree", "conflicted"],
  ] as const)("%s on %s draws %s", (status, side, icon) => {
    expect(fileState(f({ status }), side).icon).toBe(icon);
  });

  it("the tooltip of a submodule explains what staging does", () => {
    const state = fileState(f({ mode: "submodule", submodule: { newCommits: true, modified: false, untracked: false } }), "worktree");
    expect(state.tooltip).not.toContain("new commits");
    expect(state.tooltip).toContain("Staging records");
  });
});

describe("sideOfRow in the one working-tree list", () => {
  it("a staged-only row reads its index side", () => {
    expect(sideOfRow("staged", "worktree")).toBe("index");
  });
  it("a partly staged row reads what is left to stage", () => {
    expect(sideOfRow("partly", "index")).toBe("worktree");
  });
  it("a row of a plain pane keeps its pane's side", () => {
    expect(sideOfRow(undefined, "commit")).toBe("commit");
  });
});
