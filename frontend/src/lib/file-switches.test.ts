import { describe, expect, it } from "vitest";
import type { FileEntry } from "./ipc/bindings";
import { DEFAULT_VIEW, visibleFiles } from "./file-view";
import {
  COMMIT_VIEW,
  commitView,
  layoutToggle,
  stateSwitches,
  toolReason,
  withUnchanged,
} from "./file-switches";

function entry(path: string, status: FileEntry["status"] = "modified", oldPath: string | null = null): FileEntry {
  return { path, oldPath, status, mode: "plain", modeChange: null, similarity: null };
}

describe("stateSwitches", () => {
  it("keeps all six switches live on the working tree", () => {
    const switches = stateSwitches("worktree");
    expect(switches.map((s) => s.key)).toEqual([
      "unchanged",
      "untracked",
      "ignored",
      "modified",
      "skipped",
      "missing",
    ]);
    expect(switches.every((s) => s.reason === null)).toBe(true);
  });

  it("leaves only Unchanged and the rename sources live on a commit", () => {
    const live = stateSwitches("commit").filter((s) => s.reason === null);
    expect(live.map((s) => s.key)).toEqual(["unchanged", "renameSources"]);
  });

  it("turns Missing/Removed into the rename sources switch on a commit, in the same slot", () => {
    const switches = stateSwitches("commit");
    expect(switches[5]?.key).toBe("renameSources");
    expect(switches[5]?.title).toMatch(/source files of detected renames/);
  });

  it("says why every dead switch does nothing", () => {
    for (const s of stateSwitches("commit").filter((s) => s.reason !== null)) {
      expect(s.reason).toMatch(/\w/);
    }
  });

  it("has no unchanged files to offer on a stash", () => {
    const live = stateSwitches("stash").filter((s) => s.reason === null);
    expect(live.map((s) => s.key)).toEqual(["renameSources"]);
  });

  // `git stash -u` keeps untracked files, and the stash lists them in a section of their own.
  it("does not tell a stash it has no untracked files", () => {
    const untracked = stateSwitches("stash").find((s) => s.slot === "untracked");
    expect(untracked?.reason).toBe("Untracked files of a stash are always listed");
  });
});

describe("toolReason", () => {
  it("keeps every tool on the working tree", () => {
    for (const tool of ["separateIndex", "directories", "regex", "contents"] as const) {
      expect(toolReason("worktree", tool)).toBeNull();
    }
  });

  it("keeps the tree switch and the name filter on a commit, not the index split or contents", () => {
    expect(toolReason("commit", "directories")).toBeNull();
    expect(toolReason("commit", "regex")).toBeNull();
    expect(toolReason("commit", "separateIndex")).toMatch(/\w/);
    expect(toolReason("commit", "contents")).toMatch(/\w/);
  });
});

describe("commitView", () => {
  it("takes only the switches a commit has from what was stored", () => {
    const view = commitView({
      unchanged: true,
      renameSources: true,
      directories: true,
      regex: true,
      modified: false,
      missing: false,
      untracked: false,
      contents: true,
    });
    expect(view).toEqual({
      ...COMMIT_VIEW,
      unchanged: true,
      renameSources: true,
      directories: true,
      regex: true,
    });
  });

  it("never hides a file the commit changed", () => {
    const changed = [entry("a", "modified"), entry("b", "deleted"), entry("c", "added")];
    expect(visibleFiles(changed, commitView(null))).toHaveLength(3);
  });

  it("falls back to the defaults for anything unreadable", () => {
    expect(commitView("junk")).toEqual(COMMIT_VIEW);
  });

  it("starts where the working tree view starts for the tree switch", () => {
    expect(COMMIT_VIEW.directories).toBe(DEFAULT_VIEW.directories);
  });
});

describe("withUnchanged", () => {
  it("adds every tree file the commit did not change, as unchanged", () => {
    const files = withUnchanged([entry("b.txt")], ["a.txt", "b.txt", "c/d.txt"]);
    expect(files.map((f) => [f.path, f.status])).toEqual([
      ["b.txt", "modified"],
      ["a.txt", "unchanged"],
      ["c/d.txt", "unchanged"],
    ]);
  });

  it("returns the change list untouched without a tree", () => {
    const changed = [entry("b.txt")];
    expect(withUnchanged(changed, null)).toBe(changed);
  });
});

describe("a comparison of two commits", () => {
  it("has the rename sources but no unchanged files to offer", () => {
    const live = stateSwitches("compare").filter((s) => s.reason === null);
    expect(live.map((s) => s.key)).toEqual(["renameSources"]);
    expect(toolReason("compare", "separateIndex")).toMatch(/comparison/);
  });
});

// Two buttons side by side for one choice (#30): one button now, showing what it switches to.
describe("the directories button", () => {
  it("offers directories while the list is flat, and the flat list while it has them", () => {
    expect(layoutToggle(false)).toEqual({ icon: "tree", title: "Show Directories", next: true });
    expect(layoutToggle(true)).toEqual({ icon: "flat", title: "Show Flat List", next: false });
  });
});
