import { describe, expect, it } from "vitest";
import { branchMenu, commitMenu, fileMenu, refMenu, repoMenu } from "./context-menu";

describe("commitMenu", () => {
  const items = commitMenu({ onRemote: false });

  it("offers the actions a commit supports", () => {
    const ids = items.map((item) => item.id);
    expect(ids).toContain("cherry-pick");
    expect(ids).toContain("revert");
    expect(ids).toContain("split-off");
    expect(ids).toContain("rebase-i");
    expect(ids).toContain("copy-sha");
  });

  it("gives every item a label a human can read", () => {
    for (const item of items.filter((i) => !i.separator)) {
      expect(item.label.length).toBeGreaterThan(2);
      expect(item.label).not.toMatch(/^[a-z-]+$/);
    }
  });

  it("keeps history-rewriting actions available but marked on a published commit", () => {
    const published = commitMenu({ onRemote: true });
    const split = published.find((item) => item.id === "split-off");
    expect(split?.enabled).toBe(true);
    expect(split?.label).toMatch(/force-push/i);
  });

  it("separates the destructive group from the rest", () => {
    expect(items.some((item) => item.separator)).toBe(true);
  });
});

describe("branchMenu", () => {
  it("does not offer to check out the branch that is already out", () => {
    const item = branchMenu({ isHead: true, hasUpstream: true }).find(
      (entry) => entry.id === "checkout",
    );
    expect(item?.enabled).toBe(false);
  });

  it("does not offer to delete the branch that is checked out", () => {
    const item = branchMenu({ isHead: true, hasUpstream: true }).find(
      (entry) => entry.id === "delete-branch",
    );
    expect(item?.enabled).toBe(false);
  });

  it("offers pull only when there is an upstream to pull from", () => {
    expect(
      branchMenu({ isHead: true, hasUpstream: false }).find((e) => e.id === "pull")?.enabled,
    ).toBe(false);
    expect(
      branchMenu({ isHead: true, hasUpstream: true }).find((e) => e.id === "pull")?.enabled,
    ).toBe(true);
  });
});

describe("refMenu", () => {
  const ids = (items: ReturnType<typeof refMenu>) =>
    items.filter((entry) => !entry.separator).map((entry) => entry.id);

  it("offers branch actions for a local branch", () => {
    const menu = refMenu({ kind: "local", isHead: false, hasUpstream: true });
    expect(ids(menu)).toContain("checkout");
    expect(ids(menu)).toContain("delete-branch");
  });

  it("offers a tag its own actions, not a branch's", () => {
    const menu = refMenu({ kind: "tag", isHead: false, hasUpstream: false });
    expect(ids(menu)).toEqual(["checkout-tag", "delete-tag", "copy-sha"]);
  });

  it("offers a stash apply, pop and drop", () => {
    expect(ids(refMenu({ kind: "stash", isHead: false, hasUpstream: false }))).toEqual([
      "apply-stash",
      "pop-stash",
      "drop-stash",
    ]);
  });

  it("offers a lost commit the way back", () => {
    expect(ids(refMenu({ kind: "lost", isHead: false, hasUpstream: false }))).toEqual([
      "restore-lost",
      "copy-sha",
    ]);
  });

  it("has nothing to offer for a heading", () => {
    expect(refMenu({ kind: "group", isHead: false, hasUpstream: false })).toEqual([]);
  });

  it("cannot check out the branch that is already checked out", () => {
    const menu = refMenu({ kind: "local", isHead: true, hasUpstream: true });
    expect(menu.find((entry) => entry.id === "checkout")?.enabled).toBe(false);
  });
});

describe("branchMenu · управление веткой", () => {
  const ids = (items: ReturnType<typeof branchMenu>) =>
    items.filter((entry) => !entry.separator).map((entry) => entry.id);

  it("offers to rename any branch, checked out or not", () => {
    expect(ids(branchMenu({ isHead: true, hasUpstream: true }))).toContain("rename-branch");
    expect(ids(branchMenu({ isHead: false, hasUpstream: false }))).toContain("rename-branch");
  });

  it("offers to set the upstream", () => {
    expect(ids(branchMenu({ isHead: false, hasUpstream: false }))).toContain("set-upstream");
  });

  it("only offers to clear an upstream that exists", () => {
    const without = branchMenu({ isHead: false, hasUpstream: false });
    expect(without.find((entry) => entry.id === "clear-upstream")?.enabled).toBe(false);
    const with_ = branchMenu({ isHead: false, hasUpstream: true });
    expect(with_.find((entry) => entry.id === "clear-upstream")?.enabled).toBe(true);
  });

  it("offers to delete a remote branch only on a remote one", () => {
    const local = refMenu({ kind: "local", isHead: false, hasUpstream: true });
    const remote = refMenu({ kind: "remote", isHead: false, hasUpstream: false });
    expect(local.map((e) => e.id)).not.toContain("delete-remote-branch");
    expect(remote.map((e) => e.id)).toContain("delete-remote-branch");
  });

  it("does not offer to check out a remote branch as if it were local", () => {
    const remote = refMenu({ kind: "remote", isHead: false, hasUpstream: false });
    expect(remote.find((entry) => entry.id === "delete-branch")).toBeUndefined();
  });
});

describe("repoMenu", () => {
  const ids = (items: ReturnType<typeof repoMenu>) =>
    items.filter((entry) => !entry.separator).map((entry) => entry.id);

  it("offers the system actions a repository row needs", () => {
    expect(ids(repoMenu({ active: false }))).toEqual([
      "repo-open",
      "repo-explorer",
      "repo-terminal",
      "repo-copy-path",
      "repo-close",
    ]);
  });

  it("does not offer to open the repository that is already open", () => {
    const menu = repoMenu({ active: true });
    expect(menu.find((entry) => entry.id === "repo-open")?.enabled).toBe(false);
  });

  it("offers to open one that is not", () => {
    const menu = repoMenu({ active: false });
    expect(menu.find((entry) => entry.id === "repo-open")?.enabled).toBe(true);
  });

  it("always allows closing, including the active one", () => {
    for (const active of [true, false]) {
      expect(repoMenu({ active }).find((entry) => entry.id === "repo-close")?.enabled).toBe(true);
    }
  });
});

describe("fileMenu", () => {
  const ids = (items: ReturnType<typeof fileMenu>) =>
    items.filter((entry) => !entry.separator).map((entry) => entry.id);
  const enabled = (items: ReturnType<typeof fileMenu>, id: string) =>
    items.find((entry) => entry.id === id)?.enabled;

  it("offers Stage on an unstaged change and Unstage on a staged one", () => {
    expect(enabled(fileMenu({ status: "modified", staged: false, count: 1, worktree: true }), "file-stage")).toBe(true);
    expect(enabled(fileMenu({ status: "modified", staged: false, count: 1, worktree: true }), "file-unstage")).toBe(
      false,
    );
    expect(enabled(fileMenu({ status: "modified", staged: true, count: 1, worktree: true }), "file-unstage")).toBe(true);
  });

  it("carries the chord the keyboard uses for the same thing", () => {
    const menu = fileMenu({ status: "modified", staged: false, count: 1, worktree: true });
    expect(menu.find((entry) => entry.id === "file-stage")?.accelerator).toBe("CmdOrCtrl+T");
    expect(menu.find((entry) => entry.id === "file-discard")?.accelerator).toBe("CmdOrCtrl+Z");
  });

  it("does not offer to discard an untracked file, which Git cannot restore", () => {
    expect(enabled(fileMenu({ status: "untracked", staged: false, count: 1, worktree: true }), "file-discard")).toBe(
      false,
    );
    expect(enabled(fileMenu({ status: "modified", staged: false, count: 1, worktree: true }), "file-discard")).toBe(
      true,
    );
  });

  it("offers Ignore and Delete only for what Git is not tracking", () => {
    const untracked = fileMenu({ status: "untracked", staged: false, count: 1, worktree: true });
    expect(enabled(untracked, "file-ignore")).toBe(true);
    expect(enabled(untracked, "file-delete")).toBe(true);
    const tracked = fileMenu({ status: "modified", staged: false, count: 1, worktree: true });
    expect(enabled(tracked, "file-ignore")).toBe(false);
    expect(enabled(tracked, "file-delete")).toBe(false);
  });

  it("cannot blame a file that has no history yet or is gone", () => {
    expect(enabled(fileMenu({ status: "untracked", staged: false, count: 1, worktree: true }), "file-blame")).toBe(
      false,
    );
    expect(enabled(fileMenu({ status: "deleted", staged: false, count: 1, worktree: true }), "file-blame")).toBe(false);
    expect(enabled(fileMenu({ status: "modified", staged: false, count: 1, worktree: true }), "file-blame")).toBe(true);
  });

  it("turns off what only makes sense for one file once several are marked", () => {
    const many = fileMenu({ status: "modified", staged: false, count: 3, worktree: true });
    expect(enabled(many, "file-blame")).toBe(false);
    expect(enabled(many, "file-explorer")).toBe(false);
    expect(enabled(many, "file-stage")).toBe(true);
  });

  it("puts staging first and the destructive rows behind a separator", () => {
    const menu = fileMenu({ status: "modified", staged: false, count: 1, worktree: true });
    expect(ids(menu)[0]).toBe("file-stage");
    const discard = menu.findIndex((entry) => entry.id === "file-discard");
    expect(menu.slice(0, discard).some((entry) => entry.separator)).toBe(true);
  });

  it("never draws two separators in a row or ends on one", () => {
    for (const status of ["modified", "untracked", "deleted", "conflicted"] as const) {
      const menu = fileMenu({ status, staged: false, count: 1, worktree: true });
      expect(menu.at(0)?.separator).toBe(false);
      expect(menu.at(-1)?.separator).toBe(false);
      expect(menu.some((entry, at) => entry.separator && menu[at + 1]?.separator)).toBe(false);
    }
  });
});

describe("fileMenu for a commit", () => {
  it("leaves out everything that would touch the working tree", () => {
    const menu = fileMenu({ status: "modified", staged: false, count: 1, worktree: false });
    const ids = menu.map((entry) => entry.id);
    expect(ids).not.toContain("file-stage");
    expect(ids).not.toContain("file-discard");
    expect(ids).not.toContain("file-delete");
  });

  it("still offers what reading a file is for", () => {
    const menu = fileMenu({ status: "modified", staged: false, count: 1, worktree: false });
    expect(menu.find((entry) => entry.id === "file-blame")?.enabled).toBe(true);
    expect(menu.find((entry) => entry.id === "file-copy-path")?.enabled).toBe(true);
  });
});
