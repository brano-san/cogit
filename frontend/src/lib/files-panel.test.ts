import { describe, expect, it } from "vitest";
import type { FileEntry } from "$lib/ipc";
import { emptyText, filesPanelList, mergedSide, mergeIndex, stagedShown, type FilesPanelInput } from "./files-panel";

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

// "The working tree is clean." showed while the list was still being read, and again when
// reading it failed.
describe("what an empty Files list says", () => {
  it("says nothing until the list has been read", () => {
    expect(emptyText({ settled: false, failed: false }, "The working tree is clean.")).toBe("");
  });

  it("says the list is empty once it has been read", () => {
    expect(emptyText({ settled: true, failed: false }, "The working tree is clean.")).toBe(
      "The working tree is clean.",
    );
  });

  it("does not call a list that could not be read empty", () => {
    expect(emptyText({ settled: true, failed: true }, "The working tree is clean.")).not.toContain("clean");
  });
});

function row(path: string, status: FileEntry["status"], extra: Partial<FileEntry> = {}): FileEntry {
  return { path, oldPath: null, status, mode: "plain", modeChange: null, similarity: null, ...extra };
}

// Separate Working Tree and Index off still showed Unstaged and Staged, one above the other,
// and a partly staged file twice (#32).
describe("the working tree as one list", () => {
  it("lists each path once, whichever side its change is on", () => {
    const merged = mergeIndex([row("a.txt", "modified"), row("b.txt", "modified")], [row("b.txt", "modified"), row("c.txt", "added")]);
    expect(merged.map((file) => [file.path, file.indexState ?? null])).toEqual([
      ["a.txt", null],
      ["b.txt", "partly"],
      ["c.txt", "staged"],
    ]);
  });

  it("keeps what the index says of a file changed again on disk: added, renamed", () => {
    const merged = mergeIndex(
      [row("new.txt", "modified"), row("moved.txt", "modified")],
      [row("new.txt", "added"), row("moved.txt", "renamed", { oldPath: "old.txt", similarity: 90 })],
    );
    expect(merged.map((file) => [file.path, file.status, file.oldPath])).toEqual([
      ["new.txt", "added", null],
      ["moved.txt", "renamed", "old.txt"],
    ]);
  });

  it("says a staged file is gone from disk when it is", () => {
    const [merged] = mergeIndex([row("a.txt", "deleted")], [row("a.txt", "added")]);
    expect(merged).toMatchObject({ status: "deleted", indexState: "partly" });
  });

  it("opens the side of the diff that still has a change to stage", () => {
    const unstaged = [row("a.txt", "modified")];
    expect(mergedSide("a.txt", unstaged)).toBe("worktree");
    expect(mergedSide("c.txt", unstaged)).toBe("index");
  });

  it("tells Commit What You See which staged files the one list shows", () => {
    const staged = [row("b.txt", "modified"), row("c.txt", "added")];
    expect(stagedShown([["a.txt", "b.txt"]], false, staged)).toEqual(["b.txt"]);
    expect(stagedShown([["a.txt"], ["b.txt", "c.txt"]], true, staged)).toEqual(["b.txt", "c.txt"]);
    expect(stagedShown([["a.txt"]], true, staged)).toEqual([]);
  });
});
