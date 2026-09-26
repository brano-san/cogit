import { describe, expect, it } from "vitest";
import type { Branch, Tag } from "./ipc";
import type { RefNode } from "./ref-nodes";
import { checkoutPlan, nodeTarget } from "./ref-checkout";

const branch = (name: string, kind: "local" | "remote"): Branch => ({
  name,
  fullName: kind === "local" ? `refs/heads/${name}` : `refs/remotes/${name}`,
  kind,
  oid: "b".repeat(40),
  isHead: false,
  upstream: null,
  ahead: 0,
  behind: 0,
});
const tag = (name: string, pointsToCommit = true): Tag => ({
  name,
  fullName: `refs/tags/${name}`,
  oid: "c".repeat(40),
  isAnnotated: false,
  pointsToCommit,
});
const node = (over: Partial<RefNode>): RefNode => ({ id: "n", kind: "local", label: "n", depth: 0, ...over });

describe("nodeTarget", () => {
  it("stands for the branch of a branch row, local or remote", () => {
    const remote = branch("origin/feature", "remote");
    expect(nodeTarget(node({ kind: "remote", branch: remote, oid: remote.oid }), [])).toEqual({
      ref: { kind: "remote", name: "origin/feature", isHead: false },
      branch: remote,
      tag: null,
      oid: remote.oid,
    });
  });

  it("finds a tag row's tag by its full name, and no commit for a tag on a tree", () => {
    const tree = tag("docs", false);
    const found = nodeTarget(node({ kind: "tag", label: "docs", rev: "refs/tags/docs" }), [tree]);
    expect(found?.tag).toBe(tree);
    expect(found?.oid).toBeNull();
  });

  it("stands for nothing on a row that is not a ref", () => {
    expect(nodeTarget(node({ kind: "stash" }), [])).toBeNull();
  });
});

// A double click on origin/feature did nothing, and one on a tag detached HEAD without the
// question the menu's Check Out asks.
describe("checkoutPlan", () => {
  it("switches to a local branch", () => {
    const local = branch("topic", "local");
    expect(checkoutPlan(nodeTarget(node({ branch: local, oid: local.oid }), [])!, [])).toEqual({
      kind: "switch",
      branch: local,
    });
  });

  it("switches to the local branch of a remote one", () => {
    const remote = branch("origin/feature", "remote");
    const plan = checkoutPlan(nodeTarget(node({ kind: "remote", branch: remote, oid: remote.oid }), [])!, ["origin"]);
    expect(plan).toEqual({ kind: "switch", branch: { ...remote, kind: "local", name: "feature" } });
  });

  it("detaches HEAD at a tag, naming it for the question", () => {
    const release = tag("v1.0");
    const plan = checkoutPlan(nodeTarget(node({ kind: "tag", tag: release, label: "v1.0" }), [release])!, []);
    expect(plan).toEqual({ kind: "detach", oid: release.oid, what: "tag v1.0" });
  });

  it("has nothing to check out for a tag on a tree", () => {
    const tree = tag("docs", false);
    expect(checkoutPlan(nodeTarget(node({ kind: "tag", tag: tree }), [tree])!, [])).toBeNull();
  });

  it("detaches HEAD at a bare commit", () => {
    expect(checkoutPlan({ ref: null, branch: null, oid: "d".repeat(40) }, [])).toEqual({
      kind: "detach",
      oid: "d".repeat(40),
      what: "commit ddddddd",
    });
  });
});

describe("nodeTarget of a branch held by another worktree", () => {
  it("names the worktree, so the menu can refuse Delete", () => {
    const local = branch("topic", "local");
    const found = nodeTarget(node({ branch: local, oid: local.oid, worktree: { path: "D:/work/topic", state: "synced" } }), []);
    expect(found?.ref?.worktree).toBe("D:/work/topic");
    expect(nodeTarget(node({ branch: local, oid: local.oid }), [])?.ref).not.toHaveProperty("worktree");
  });
});
