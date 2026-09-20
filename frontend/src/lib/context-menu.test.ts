import { describe, expect, it } from "vitest";
import { commitMenu, fileMenu, branchMenu } from "./context-menu";

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
