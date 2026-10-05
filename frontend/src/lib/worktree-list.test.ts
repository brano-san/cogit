import { describe, expect, it } from "vitest";
import type { Branch, FileEntry, WorktreeEntry } from "$lib/ipc";
import {
  changeCounts,
  compactCounts,
  middlePath,
  hasStale,
  linkedCount,
  listedRows,
  moveBlocked,
  othersToWatch,
  prunable,
  removeBlocked,
  worktreeRowKey,
  pruneAllButton,
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
    changed: 0,
    untracked: 0,
    bare: false,
    hasSubmodules: false,
    ...over,
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
  // "main" beside the branch master read as one more branch.
  it("marks the main copy as primary, not as main", () => {
    const tags = worktreeTags(entry({ isMain: true }));
    expect(tags.map((tag) => tag.label)).toEqual(["primary"]);
    expect(tags[0]?.tooltip).toMatch(/^Main worktree/);
  });

  it("marks the one open in Cogit and a bare main one", () => {
    expect(worktreeTags(entry({ isCurrent: true })).map((tag) => tag.id)).toEqual(["open"]);
    expect(worktreeTags(entry({ isMain: true, bare: true })).map((tag) => tag.id)).toEqual(["primary", "bare"]);
  });

  it("says a missing one is prunable unless it is locked", () => {
    expect(worktreeTags(entry({ missing: true }))[0]?.tooltip).toContain("prunable");
    const locked = worktreeTags(entry({ missing: true, locked: "" })).find((tag) => tag.id === "missing");
    expect(locked?.tooltip).not.toContain("prunable");
    expect(locked?.tooltip).toContain("locked");
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

  it("leaves uncommitted changes to the counters, not to a badge", () => {
    expect(worktreeTags(entry({ dirty: true }))).toEqual([]);
  });

  it("counts the changes compactly, in words in the tooltip", () => {
    expect(compactCounts(entry({ dirty: true, changed: 93, untracked: 2 }))).toEqual({
      text: "93 ✎ · 2 ?",
      tooltip: "Uncommitted changes: 93 changed, 2 untracked",
    });
    expect(compactCounts(entry({ dirty: false }))).toBeNull();
    expect(compactCounts(entry({ dirty: true, missing: true }))).toBeNull();
    expect(changeCounts(entry({ dirty: true, untracked: 1 }))).toBe("1 untracked");
  });
});

describe("middlePath", () => {
  it("keeps the first and the last folder and leaves the middle out", () => {
    expect(middlePath("D:/Work1/projects/dtv_device")).toBe("D:/…/dtv_device");
    expect(middlePath("/home/me/src/x")).toBe("/…/x");
  });

  it("keeps a short path whole", () => {
    expect(middlePath("D:/dtv_device")).toBe("D:/dtv_device");
    expect(middlePath("D:/Work1/dtv_device")).toBe("D:/Work1/dtv_device");
  });
});

describe("listedRows", () => {
  const main = entry({ isMain: true, isCurrent: true });

  it("shows nothing while the main worktree is the only one", () => {
    expect(listedRows([main])).toEqual([]);
    expect(listedRows([])).toEqual([]);
  });

  it("shows the main one too once there is a linked one", () => {
    const linked = entry({ path: "E:/w/x" });
    expect(listedRows([main, linked])).toEqual([main, linked]);
  });
});

// The header said "Worktrees (2)" for the main worktree and one linked one.
describe("linkedCount", () => {
  it("counts the linked worktrees, not the main one", () => {
    expect(linkedCount([entry({ isMain: true })])).toBe(0);
    expect(linkedCount([entry({ isMain: true }), entry(), entry({ missing: true })])).toBe(2);
  });
});

describe("what the panel offers", () => {
  it("enables Prune All only when something is stale", () => {
    expect(hasStale([entry(), entry({ missing: true })])).toBe(true);
    expect(hasStale([entry()])).toBe(false);
  });

  // Git keeps a locked registration, gone or not: Prune All on it did nothing.
  it("counts as prunable only a missing worktree that is not locked", () => {
    const gone = entry({ missing: true, path: "E:/w/gone" });
    const kept = entry({ missing: true, locked: "", path: "E:/w/kept" });
    expect(prunable([entry(), gone, kept])).toEqual([gone]);
    expect(hasStale([entry(), kept])).toBe(false);
  });

  it("says how many Prune All forgets, and why it is off", () => {
    const gone = entry({ missing: true });
    expect(pruneAllButton([entry(), gone, gone])).toEqual({
      label: "Prune All (2)…",
      disabled: false,
      tip: "Forget 2 worktrees whose folder is gone",
    });
    expect(pruneAllButton([entry()])).toMatchObject({ label: "Prune All…", disabled: true });
    expect(pruneAllButton([entry()]).tip).toMatch(/^Nothing to prune/);
    expect(pruneAllButton([entry({ missing: true, locked: "" })]).tip).toContain("locked");
  });

  it("never offers to remove the main copy or a missing one", () => {
    expect(removable(entry())).toBe(true);
    expect(removable(entry({ isMain: true }))).toBe(false);
    expect(removable(entry({ missing: true }))).toBe(false);
    expect(removable(entry({ isCurrent: true }))).toBe(false);
    expect(removable(undefined)).toBe(false);
  });

  it("removes a locked one, with --force", () => {
    expect(removable(entry({ locked: "usb" }))).toBe(true);
    expect(removalNeeds(entry({ locked: "usb" }), null)).toMatchObject({ locked: true, force: true });
    expect(moveBlocked(entry({ locked: "usb" }))).toContain("unlock");
    expect(moveBlocked(entry())).toBeNull();
    expect(removeBlocked(entry({ isMain: true }))).toBe("the main worktree");
  });
});

describe("worktreeRowKey", () => {
  it("opens on Enter unless the folder is gone", () => {
    expect(worktreeRowKey("Enter", entry())).toBe("open");
    expect(worktreeRowKey("Enter", entry({ missing: true }))).toBeNull();
  });

  it("asks to remove on Delete only what can be removed", () => {
    expect(worktreeRowKey("Delete", entry())).toBe("remove");
    expect(worktreeRowKey("Delete", entry({ isMain: true }))).toBeNull();
    expect(worktreeRowKey("Delete", entry({ isCurrent: true }))).toBeNull();
    expect(worktreeRowKey("x", entry())).toBeNull();
  });
});

describe("the Remove Worktree dialog", () => {
  const change = { path: "a.txt", status: "modified" } as FileEntry;

  it("waits for the changes before it asks anything", () => {
    expect(removalNeeds(entry(), null)).toEqual({ dirty: false, submodules: false, locked: false, force: false });
  });

  it("asks for --force when there are uncommitted changes", () => {
    expect(removalNeeds(entry(), [change])).toEqual({ dirty: true, submodules: false, locked: false, force: true });
  });

  // Git refuses a clean worktree with submodules checked out unless forced.
  it("asks for --force when submodules are checked out in it, clean as it is", () => {
    expect(removalNeeds(entry({ hasSubmodules: true }), [])).toEqual({
      dirty: false,
      submodules: true,
      locked: false,
      force: true,
    });
  });

  it("removes a clean worktree without submodules plainly", () => {
    expect(removalNeeds(entry(), [])).toEqual({ dirty: false, submodules: false, locked: false, force: false });
  });
});

describe("worktreeMarks", () => {
  const local = (name: string, over: Partial<Branch> = {}): Branch => ({
    name,
    fullName: `refs/heads/${name}`,
    kind: "local",
    oid: OID,
    isHead: false,
    upstream: `origin/${name}`, pushRemote: null, pushTarget: null,
    ahead: 0,
    behind: 0,
    ...over,
  });

  const tracking = (name: string): Branch =>
    local(name, { kind: "remote", fullName: `refs/remotes/${name}`, upstream: null });

  it("marks a branch another worktree holds, with its path", () => {
    const marks = worktreeMarks(
      [
        entry({ branch: "main", isMain: true, isCurrent: true, path: "E:/w/main" }),
        entry({ branch: "feature", path: "E:/w/feature" }),
      ],
      [local("main"), local("feature"), tracking("origin/main"), tracking("origin/feature")],
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
        tracking("origin/ahead"),
        tracking("origin/behind"),
      ],
    );
    expect(marks.get("ahead")?.state).toBe("unpushed");
    expect(marks.get("local-only")?.state).toBe("unpushed");
    expect(marks.get("unknown")?.state).toBe("unpushed");
    expect(marks.get("behind")?.state).toBe("synced");
  });

  it("does not call a worktree pushed when its upstream is gone from the server", () => {
    const marks = worktreeMarks([entry({ branch: "feature", path: "E:/w/feature" })], [local("feature")]);
    expect(marks.get("feature")?.state).toBe("unpushed");
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

// An edit in the folder of a linked worktree left its mark grey and its tag clean: only the
// repository on screen is watched, and the list was read only on its events.
describe("othersToWatch", () => {
  it("is true while another worktree's folder is there to change", () => {
    expect(othersToWatch([entry({ isMain: true, isCurrent: true }), entry()])).toBe(true);
  });

  it("is false with only the one on screen, or others whose folder is gone", () => {
    expect(othersToWatch([entry({ isMain: true, isCurrent: true })])).toBe(false);
    expect(othersToWatch([entry({ isCurrent: true }), entry({ missing: true })])).toBe(false);
  });
});
