import { describe, expect, it } from "vitest";
import type { FileEntry } from "$lib/ipc";
import { changeCount, emptyText, filesPanelList, mergedSide, mergeIndex, pairMemo, selectAllKeys, sameLists, stagedShown, type FilesPanelInput } from "./files-panel";

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

describe("selectAllKeys", () => {
  const panes = [["0:a", "0:b"], ["1:a", "1:c"]];
  it("stays in the touched pane", () => {
    expect(selectAllKeys(panes, 1, new Set(["0:a"]))).toEqual(["1:a", "1:c"]);
  });
  it("falls back to the pane holding a tick, then the first", () => {
    expect(selectAllKeys(panes, null, new Set(["1:c"]))).toEqual(["1:a", "1:c"]);
    expect(selectAllKeys(panes, null, new Set())).toEqual(["0:a", "0:b"]);
    expect(selectAllKeys([], null, new Set())).toEqual([]);
  });
  it("leaves a touched pane that emptied or went", () => {
    expect(selectAllKeys([[], ["1:a"]], 0, new Set())).toEqual(["1:a"]);
    expect(selectAllKeys([["0:a"]], -1, new Set())).toEqual(["0:a"]);
  });
});

// Each click on a file made the panel build its sections anew: the content search started
// over and the LFS column read every file again, though no file had changed.
describe("sameLists", () => {
  const a: FileEntry[] = [];
  const b: FileEntry[] = [];

  it("keeps the lists it had while each one is the same", () => {
    const before = [a, b];
    expect(sameLists(before, [a, b])).toBe(before);
  });

  it("takes the new lists once one of them changed", () => {
    const next = [a, []];
    expect(sameLists([a, b], next)).toBe(next);
    expect(sameLists([a], [a, b])).toEqual([a, b]);
  });
});

// A filter over a clean tree listed 86 unchanged files under "Unstaged (86)".
describe("changeCount", () => {
  it("counts the changes a list shows, not the unchanged files beside them", () => {
    const files = [
      { path: "a", status: "modified" },
      { path: "b", status: "unchanged" },
      { path: "c", status: "untracked" },
    ] as const;
    expect(changeCount(files)).toBe(2);
    expect(changeCount([files[1]])).toBe(0);
  });
});

describe("pairMemo", () => {
  it("gives back the same value for the same two lists, and builds again when one changes", () => {
    const join = pairMemo((a: number[], b: number[]) => [...b, ...a]);
    const a = [1];
    const b = [2];
    const first = join(a, b);
    expect(join(a, b)).toBe(first);
    expect(join(a, [2])).not.toBe(first);
    expect(join([1], b)).not.toBe(first);
  });
});
