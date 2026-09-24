import { describe, expect, it } from "vitest";
import type { FileEntry } from "$lib/ipc";
import { stagesEverything } from "./stage-all";

function entry(path: string, status: FileEntry["status"] = "modified"): FileEntry {
  return { path, oldPath: null, status, mode: "plain", modeChange: null, similarity: null };
}

const unstaged = [entry("a.txt"), entry("b.txt", "deleted"), entry("new/", "untracked")];

describe("stagesEverything", () => {
  it("is every row of Unstaged, in any order", () => {
    expect(stagesEverything(["new/", "a.txt", "b.txt"], unstaged)).toBe(true);
  });

  it("is not a part of it", () => {
    expect(stagesEverything(["a.txt", "b.txt"], unstaged)).toBe(false);
  });

  it("is not a list padded with a repeat to the same length", () => {
    expect(stagesEverything(["a.txt", "a.txt", "b.txt"], unstaged)).toBe(false);
  });

  it("is not a path the list does not have", () => {
    expect(stagesEverything(["a.txt", "b.txt", "other.txt"], unstaged)).toBe(false);
  });

  it("is not an empty list, even of an empty Unstaged", () => {
    expect(stagesEverything([], [])).toBe(false);
  });

  // Named, an ignored file makes `git add` fail; `--all` would skip it without a word.
  it("is not a list with a row only a view switch shows", () => {
    for (const status of ["ignored", "skipped", "unchanged", "assumeUnchanged"] as const) {
      const shown = [...unstaged, entry("x.log", status)];
      expect(stagesEverything(shown.map((file) => file.path), shown), status).toBe(false);
    }
  });

  it("takes conflicts and renames as changes", () => {
    const rows = [entry("c.txt", "conflicted"), entry("d.txt", "renamed")];
    expect(stagesEverything(["c.txt", "d.txt"], rows)).toBe(true);
  });
});
