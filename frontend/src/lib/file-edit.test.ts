import { describe, expect, it } from "vitest";
import type { FileStatus } from "./ipc";
import { commitRoute, editRoute, openRoute } from "./file-edit";

const plain = (status: FileStatus) => ({ status, mode: "plain" as const });
const onDisk = { unstaged: true, headExists: true };
const indexOnly = { unstaged: false, headExists: true };

// Edit on an unchanged file opened a window saying "No changes".
describe("editRoute", () => {
  it("opens the file alone where there is nothing to compare", () => {
    for (const status of ["unchanged", "ignored", "assumeUnchanged", "skipped"] as const) {
      expect(editRoute(plain(status), onDisk)).toEqual({ kind: "editor" });
    }
  });

  it("opens a change as a diff against the index, the file editable", () => {
    for (const status of ["modified", "added", "untracked", "renamed", "copied"] as const) {
      expect(editRoute(plain(status), onDisk)).toEqual({ kind: "compare", spec: { kind: "workTreeVsIndex" } });
    }
  });

  it("compares with HEAD when the index already holds the whole change", () => {
    expect(editRoute(plain("added"), indexOnly)).toEqual({ kind: "compare", spec: { kind: "commitVsWorkTree", oid: "HEAD" } });
    expect(editRoute(plain("added"), { unstaged: false, headExists: false })).toEqual({
      kind: "compare",
      spec: { kind: "workTreeVsIndex" },
    });
  });

  it("resolves a conflict in the Conflict Solver", () => {
    expect(editRoute(plain("conflicted"), onDisk)).toEqual({ kind: "solver" });
  });

  it("has nothing to edit for a file not on disk or a submodule", () => {
    expect(editRoute(plain("deleted"), onDisk)).toEqual({ kind: "refused", reason: "not on disk" });
    expect(editRoute(plain("sparse"), onDisk)).toEqual({ kind: "refused", reason: "not on disk" });
    expect(editRoute({ status: "modified", mode: "submodule" }, onDisk)).toEqual({ kind: "refused", reason: "a submodule" });
  });
});

// Enter on a row that was only focused opened the side the Diff panel showed last.
describe("openRoute", () => {
  it("opens the side of the list the row is in", () => {
    expect(openRoute(plain("modified"), "worktree")).toEqual({ kind: "compare", spec: { kind: "workTreeVsIndex" } });
    expect(openRoute(plain("modified"), "index")).toEqual({ kind: "compare", spec: { kind: "indexVsHead" } });
    expect(openRoute(plain("deleted"), "worktree")).toEqual({ kind: "compare", spec: { kind: "workTreeVsIndex" } });
  });

  it("opens an unchanged or ignored file on its own", () => {
    expect(openRoute(plain("unchanged"), "worktree")).toEqual({ kind: "editor" });
    expect(openRoute(plain("ignored"), "worktree")).toEqual({ kind: "editor" });
  });

  it("sends a conflict to the Solver and a submodule to its repository", () => {
    expect(openRoute(plain("conflicted"), "worktree")).toEqual({ kind: "solver" });
    expect(openRoute({ status: "unchanged", mode: "submodule" }, "worktree")).toEqual({ kind: "module" });
  });
});

// A double-click on a past commit's file opened a window saying it needs a file to compare.
describe("commitRoute", () => {
  it("opens the commit's change to the file, whatever the Diff panel showed last", () => {
    expect(commitRoute("plain", "abc123")).toEqual({ kind: "compare", spec: { kind: "commitVsParent", oid: "abc123" } });
  });

  it("opens a submodule in its repository and has nothing to open without a commit", () => {
    expect(commitRoute("submodule", "abc123")).toEqual({ kind: "module" });
    expect(commitRoute("plain", null)).toEqual({ kind: "refused", reason: "no commit" });
  });
});
