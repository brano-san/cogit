import { describe, expect, it } from "vitest";
import { commitMenu, fileMenu, branchMenu, refMenu } from "./context-menu";

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

describe("fileMenu", () => {
  it("offers staging for an unstaged file", () => {
    const ids = fileMenu({ staged: false }).map((item) => item.id);
    expect(ids).toContain("stage");
    expect(ids).toContain("discard");
    expect(ids).not.toContain("unstage");
  });

  it("offers unstaging for a staged one", () => {
    const ids = fileMenu({ staged: true }).map((item) => item.id);
    expect(ids).toContain("unstage");
    expect(ids).not.toContain("stage");
  });

  it("always offers the harmless actions", () => {
    for (const staged of [true, false]) {
      const ids = fileMenu({ staged }).map((item) => item.id);
      expect(ids).toContain("copy-path");
      expect(ids).toContain("blame");
    }
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
