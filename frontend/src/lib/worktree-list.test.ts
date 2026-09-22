import { describe, expect, it } from "vitest";
import type { Branch, WorktreeEntry } from "$lib/ipc";
import {
  addProblem,
  branchChoices,
  hasStale,
  removable,
  worktreeTags,
  worktreeWhere,
} from "./worktree-list";

const OID = "20483bd723e2be25a60f7b2bf74016a444a16df0";

function entry(over: Partial<WorktreeEntry> = {}): WorktreeEntry {
  return {
    path: "E:/Work1/dtv_device_master",
    name: "dtv_device_master",
    branch: "master",
    head: OID,
    isMain: false,
    isCurrent: false,
    locked: null,
    missing: false,
    dirty: false,
    ...over,
  };
}

function branch(name: string): Branch {
  return {
    name,
    fullName: `refs/heads/${name}`,
    kind: "local",
    oid: OID,
    isHead: false,
    upstream: null,
    ahead: 0,
    behind: 0,
  };
}

describe("worktreeWhere", () => {
  it("names the branch", () => {
    expect(worktreeWhere(entry())).toBe("master");
  });

  it("says where a detached worktree is", () => {
    expect(worktreeWhere(entry({ branch: null }))).toBe("detached at 20483bd");
  });

  it("says nothing it cannot know about a missing one", () => {
    expect(worktreeWhere(entry({ branch: null, head: "", missing: true }))).toBe("");
  });
});

describe("worktreeTags", () => {
  it("marks the main copy", () => {
    expect(worktreeTags(entry({ isMain: true })).map((tag) => tag.id)).toEqual(["main"]);
  });

  it("explains missing and says what to do about it", () => {
    const [tag] = worktreeTags(entry({ missing: true, dirty: true }));
    expect(tag?.id).toBe("missing");
    expect(tag?.tooltip).toMatch(/Prune .* Repair/);
  });

  it("gives the lock reason", () => {
    const [tag] = worktreeTags(entry({ locked: "on a usb stick" }));
    expect(tag?.tooltip).toContain("on a usb stick");
  });

  it("marks uncommitted changes", () => {
    expect(worktreeTags(entry({ dirty: true })).map((tag) => tag.id)).toEqual(["dirty"]);
  });
});

describe("what the panel offers", () => {
  it("enables Prune All only when something is stale", () => {
    expect(hasStale([entry(), entry({ missing: true })])).toBe(true);
    expect(hasStale([entry()])).toBe(false);
  });

  it("never offers to remove the main copy or a missing one", () => {
    expect(removable(entry())).toBe(true);
    expect(removable(entry({ isMain: true }))).toBe(false);
    expect(removable(entry({ missing: true }))).toBe(false);
    expect(removable(entry({ isCurrent: true }))).toBe(false);
    expect(removable(undefined)).toBe(false);
  });
});

describe("the Add Worktree dialog", () => {
  const choices = branchChoices(
    [branch("master"), branch("feature/14340_new_toolchain"), { ...branch("origin/x"), kind: "remote" }],
    [
      entry({ isMain: true, path: "E:/Work1/dtv_device", branch: "feature/14340_new_toolchain" }),
      entry(),
    ],
  );

  it("lists local branches and where each is taken", () => {
    expect(choices).toEqual([
      { name: "master", heldBy: "E:/Work1/dtv_device_master" },
      { name: "feature/14340_new_toolchain", heldBy: "E:/Work1/dtv_device" },
    ]);
  });

  it("refuses a branch another worktree holds, and says which", () => {
    expect(addProblem({ folder: "E:/wt", create: false, branch: "master", choices })).toMatch(
      /checked out in E:\/Work1\/dtv_device_master/,
    );
  });

  it("needs a folder first", () => {
    expect(addProblem({ folder: " ", create: true, branch: "x", choices })).toMatch(/folder/);
  });

  it("sends a new branch that already exists to the existing list", () => {
    expect(addProblem({ folder: "E:/wt", create: true, branch: "master", choices })).toMatch(
      /already exists/,
    );
  });

  it("goes ahead with a fresh branch name", () => {
    expect(addProblem({ folder: "E:/wt", create: true, branch: "spike", choices })).toBeNull();
  });
});
