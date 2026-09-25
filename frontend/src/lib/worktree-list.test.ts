import { describe, expect, it } from "vitest";
import type { Branch, FileEntry, WorktreeEntry } from "$lib/ipc";
import {
  addProblem,
  branchChoices,
  hasStale,
  removable,
  removalNeeds,
  worktreeTags,
  worktreeWhere,
  worktreeMarks,
  worktreeMarkTooltip,
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
    hasSubmodules: false,
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

describe("the Remove Worktree dialog", () => {
  const change = { path: "a.txt", status: "modified" } as FileEntry;

  it("waits for the changes before it asks anything", () => {
    expect(removalNeeds(entry(), null)).toEqual({ dirty: false, submodules: false, force: false });
  });

  it("asks for --force when there are uncommitted changes", () => {
    expect(removalNeeds(entry(), [change])).toEqual({ dirty: true, submodules: false, force: true });
  });

  // Git refuses a clean worktree with submodules checked out unless forced.
  it("asks for --force when submodules are checked out in it, clean as it is", () => {
    expect(removalNeeds(entry({ hasSubmodules: true }), [])).toEqual({
      dirty: false,
      submodules: true,
      force: true,
    });
  });

  it("removes a clean worktree without submodules plainly", () => {
    expect(removalNeeds(entry(), [])).toEqual({ dirty: false, submodules: false, force: false });
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

describe("worktreeMarks", () => {
  const local = (name: string, over: Partial<Branch> = {}): Branch => ({
    name,
    fullName: `refs/heads/${name}`,
    kind: "local",
    oid: OID,
    isHead: false,
    upstream: `origin/${name}`,
    ahead: 0,
    behind: 0,
    ...over,
  });

  it("marks a branch another worktree holds, with its path", () => {
    const marks = worktreeMarks(
      [
        entry({ branch: "main", isMain: true, isCurrent: true, path: "E:/w/main" }),
        entry({ branch: "feature", path: "E:/w/feature" }),
      ],
      [local("main"), local("feature")],
    );
    expect(marks.get("feature")).toEqual({ path: "E:/w/feature", state: "synced" });
  });

  it("leaves out the worktree on screen and a detached one", () => {
    const marks = worktreeMarks([
      entry({ branch: "main", isCurrent: true }),
      entry({ branch: null, path: "E:/w/detached" }),
    ]);
    expect(marks.size).toBe(0);
  });

  it("says whether the worktree has changes or is missing", () => {
    const marks = worktreeMarks(
      [entry({ branch: "dirty", dirty: true }), entry({ branch: "gone", missing: true, dirty: true })],
      [local("dirty"), local("gone")],
    );
    expect(marks.get("dirty")?.state).toBe("changes");
    expect(marks.get("gone")?.state).toBe("missing");
  });

  it("calls a clean worktree synced only when its branch has nothing left to push", () => {
    const marks = worktreeMarks(
      [
        entry({ branch: "ahead", path: "E:/w/ahead" }),
        entry({ branch: "local-only", path: "E:/w/local-only" }),
        entry({ branch: "unknown", path: "E:/w/unknown" }),
        entry({ branch: "behind", path: "E:/w/behind" }),
      ],
      [
        local("ahead", { ahead: 2 }),
        local("local-only", { upstream: null }),
        local("behind", { behind: 3 }),
      ],
    );
    expect(marks.get("ahead")?.state).toBe("unpushed");
    expect(marks.get("local-only")?.state).toBe("unpushed");
    expect(marks.get("unknown")?.state).toBe("unpushed");
    expect(marks.get("behind")?.state).toBe("synced");
  });

  it("puts the path and the state in the tooltip", () => {
    expect(worktreeMarkTooltip({ path: "E:/w/x", state: "missing" })).toBe(
      "Checked out in the worktree E:/w/x; its folder is missing",
    );
    expect(worktreeMarkTooltip({ path: "E:/w/x", state: "synced" })).toBe(
      "Checked out in the worktree E:/w/x; it is clean and pushed",
    );
    expect(worktreeMarkTooltip({ path: "E:/w/x", state: "unpushed" })).toBe(
      "Checked out in the worktree E:/w/x; it is clean, with commits not pushed",
    );
  });
});
