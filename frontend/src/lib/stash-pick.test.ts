import { describe, expect, it } from "vitest";
import type { FileEntry, FileStatus, StashContents } from "$lib/ipc";
import { pickedPaths, stashFiles, stashFileTag } from "./stash-pick";

function entry(path: string, status: FileStatus, oldPath?: string): FileEntry {
  return { path, status, oldPath: oldPath ?? null, mode: "plain" };
}

function contents(parts: Partial<Pick<StashContents, "worktree" | "index" | "untracked">>): StashContents {
  return {
    worktree: [],
    index: [],
    untracked: [],
    base: "b",
    worktreeRev: "w",
    indexRev: "i",
    untrackedRev: null,
    ...parts,
  } as StashContents;
}

describe("stashFiles", () => {
  it("lists every file once, sorted, untracked ones marked so", () => {
    const files = stashFiles(
      contents({
        worktree: [entry("b.txt", "modified"), entry("a.txt", "added")],
        index: [entry("a.txt", "added"), entry("staged-only.txt", "modified")],
        untracked: [entry("new.txt", "added")],
      }),
    );

    expect(files.map((file) => [file.path, file.status])).toEqual([
      ["a.txt", "added"],
      ["b.txt", "modified"],
      ["new.txt", "untracked"],
      ["staged-only.txt", "modified"],
    ]);
  });

  it("keeps a rename's source and drops a copy's", () => {
    const files = stashFiles(
      contents({ worktree: [entry("to.txt", "renamed", "from.txt"), entry("copy.txt", "copied", "orig.txt")] }),
    );

    expect(files.find((file) => file.path === "to.txt")?.oldPath).toBe("from.txt");
    expect(files.find((file) => file.path === "copy.txt")?.oldPath).toBeNull();
  });
});

describe("pickedPaths", () => {
  const files = stashFiles(
    contents({
      worktree: [entry("a.txt", "modified"), entry("to.txt", "renamed", "from.txt")],
      untracked: [entry("new.txt", "added")],
    }),
  );

  it("is null with every file ticked: a plain apply", () => {
    expect(pickedPaths(files, new Set())).toBeNull();
  });

  it("names only the ticked files", () => {
    expect(pickedPaths(files, new Set(["a.txt", "to.txt"]))).toEqual(["new.txt"]);
  });

  it("brings a rename's source along", () => {
    expect(pickedPaths(files, new Set(["a.txt"]))).toEqual(["new.txt", "to.txt", "from.txt"]);
  });

  it("is empty with nothing ticked", () => {
    expect(pickedPaths(files, new Set(["a.txt", "to.txt", "new.txt"]))).toEqual([]);
  });
});

describe("stashFileTag", () => {
  it("tags each kind of change", () => {
    expect(["added", "deleted", "untracked", "modified", "renamed"].map((s) => stashFileTag(s as FileStatus))).toEqual([
      "A",
      "D",
      "?",
      "M",
      "R",
    ]);
  });
});
