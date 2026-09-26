import { describe, expect, it } from "vitest";
import type { Branch, Head, Tag } from "./ipc";
import type { RefNode } from "./ref-nodes";
import {
  branchFacts,
  checkoutOffer,
  checkoutRequest,
  nodeTarget,
  pickProblem,
  refActivation,
  type CheckoutContext,
  type CheckoutPick,
} from "./ref-checkout";

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

  it("carries a local branch's upstream, which Stop Tracking reads", () => {
    const local = { ...branch("feature", "local"), upstream: "origin/feature" };
    expect(nodeTarget(node({ kind: "local", branch: local }), [])?.ref).toEqual({
      kind: "branch",
      name: "feature",
      isHead: false,
      upstream: "origin/feature",
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

const ctx = (branches: Branch[], head: Head | null = null): CheckoutContext => ({
  branches,
  remotes: ["origin"],
  head,
});
const remoteRow = (remote: Branch) => nodeTarget(node({ kind: "remote", branch: remote, oid: remote.oid }), [])!;
const pick = (choice: CheckoutPick["choice"], name = "", track = true): CheckoutPick => ({ choice, name, track });

// Item 40: the menu's Check Out and a double click open one Checkout dialog; what it offers
// depends on what was clicked, and where. It replaces the plan of FR-022, which switched to
// a remote branch's local one and detached at a tag after a yes-or-no question.
describe("checkoutOffer", () => {
  it("offers a local branch as it is in Branches, with what it tracks", () => {
    const local = { ...branch("topic", "local"), upstream: "origin/topic", behind: 2 };
    const offer = checkoutOffer(nodeTarget(node({ branch: local, oid: local.oid }), [])!, ctx([local]), "branches");
    expect(offer?.plain).toBe(local);
    expect(branchFacts(local)).toBe("topic is 2 commits behind origin/topic.");
    expect(checkoutRequest(offer!, pick("local"))).toEqual({
      target: { kind: "branch", name: "topic" },
      branch: "topic",
      what: "topic",
    });
  });

  it("has nothing to do for the checked-out branch", () => {
    const current = { ...branch("main", "local"), isHead: true };
    const at = nodeTarget(node({ branch: current, oid: current.oid }), [])!;
    expect(checkoutOffer(at, ctx([current]), "branches")).toBeNull();
    expect(checkoutOffer(at, ctx([current]), "graph")).toBeNull();
  });

  it("offers a new tracking branch or a detached look for a remote branch nobody tracks", () => {
    const remote = branch("origin/feature", "remote");
    const offer = checkoutOffer(remoteRow(remote), ctx([remote]), "branches")!;
    expect(offer.create).toEqual({ name: "feature", start: "refs/remotes/origin/feature", remote: "origin/feature" });
    expect(offer.detach).toEqual({ oid: remote.oid, blocked: null });
    expect(offer.local).toBeNull();
    expect(offer.initial).toBe("create");
    expect(checkoutRequest(offer, pick("create", " feature "))).toEqual({
      target: { kind: "newBranch", name: "feature", start: "refs/remotes/origin/feature", track: true },
      branch: null,
      what: "feature",
    });
    expect(checkoutRequest(offer, pick("create", "feature", false))?.target).toMatchObject({ track: false });
    expect(checkoutRequest(offer, pick("detach"))?.target).toEqual({ kind: "commit", oid: remote.oid });
  });

  it("fast-forwards the local branch that tracks it, and ticks that choice first", () => {
    const remote = { ...branch("origin/feature", "remote"), oid: "e".repeat(40) };
    const local = { ...branch("feature", "local"), upstream: "origin/feature", behind: 3 };
    const offer = checkoutOffer(remoteRow(remote), ctx([remote, local]), "branches")!;
    expect(offer.local).toMatchObject({
      name: "feature",
      to: "refs/remotes/origin/feature",
      label: "Checkout and fast-forward local branch 'feature'",
      blocked: null,
    });
    expect(offer.initial).toBe("local");
    expect(checkoutRequest(offer, pick("local"))).toEqual({
      target: { kind: "fastForward", name: "feature", to: "refs/remotes/origin/feature" },
      branch: "feature",
      what: "feature",
    });
  });

  // Git links branches by upstream alone; a branch of any name tracking it counts.
  it("finds the tracking branch by its upstream, whatever it is called", () => {
    const remote = branch("origin/feature", "remote");
    const other = { ...branch("review", "local"), upstream: "origin/feature", behind: 1 };
    const offer = checkoutOffer(remoteRow(remote), ctx([remote, other]), "branches")!;
    expect(offer.local?.name).toBe("review");
    expect(offer.create?.name).toBe("feature");
  });

  it("prefers the tracking branch of the same name when several track it", () => {
    const remote = branch("origin/feature", "remote");
    const first = { ...branch("a-review", "local"), upstream: "origin/feature" };
    const same = { ...branch("feature", "local"), upstream: "origin/feature" };
    expect(checkoutOffer(remoteRow(remote), ctx([remote, first, same]), "branches")!.local?.name).toBe("feature");
  });

  it("checks out a tracking branch as it is when there is nothing to fast-forward", () => {
    const remote = branch("origin/feature", "remote");
    const ahead = { ...branch("feature", "local"), upstream: "origin/feature", ahead: 2 };
    const offer = checkoutOffer(remoteRow(remote), ctx([remote, ahead]), "branches")!;
    expect(offer.local).toMatchObject({ to: null, label: "Check out local branch 'feature'" });
    expect(offer.local?.explanation).toContain("and 2 commits more");
    expect(checkoutRequest(offer, pick("local"))?.target).toEqual({ kind: "branch", name: "feature" });
  });

  it("never fast-forwards a tracking branch that has diverged", () => {
    const remote = branch("origin/feature", "remote");
    const diverged = { ...branch("feature", "local"), upstream: "origin/feature", ahead: 1, behind: 2 };
    const offer = checkoutOffer(remoteRow(remote), ctx([remote, diverged]), "branches")!;
    expect(offer.local).toMatchObject({ to: null, blocked: null });
    expect(offer.local?.explanation).toContain("diverged (↑1 ↓2)");
  });

  it("offers nothing more for a tracking branch already checked out and up to date", () => {
    const remote = branch("origin/main", "remote");
    const current = { ...branch("main", "local"), upstream: "origin/main", isHead: true };
    const offer = checkoutOffer(remoteRow(remote), ctx([remote, current]), "branches")!;
    expect(offer.local?.blocked).toBe("already checked out");
    expect(offer.initial).toBe("create");
  });

  // R-560: a local branch of the same name without that upstream is not linked to it.
  it("does not take a same-named branch without that upstream for the tracking one", () => {
    const remote = branch("origin/feature", "remote");
    const loose = branch("feature", "local");
    const offer = checkoutOffer(remoteRow(remote), ctx([remote, loose]), "branches")!;
    expect(offer.local).toBeNull();
    expect(pickProblem(offer, pick("create", "feature"), [remote, loose])).toBe(
      "feature already exists and does not track origin/feature. Choose another name, or set its upstream to origin/feature first.",
    );
    expect(pickProblem(offer, pick("create", "feature-2"), [remote, loose])).toBeNull();
  });

  it("offers a tag's commit detached, or a new branch there that tracks nothing", () => {
    const release = tag("v1.0");
    const at = nodeTarget(node({ kind: "tag", tag: release, label: "v1.0" }), [release])!;
    const offer = checkoutOffer(at, ctx([]), "branches")!;
    expect(offer.source).toBe("tag v1.0");
    expect(offer.create).toEqual({ name: "", start: release.oid, remote: null });
    expect(offer.initial).toBe("detach");
    expect(checkoutRequest(offer, pick("create", "hotfix", true))?.target).toEqual({
      kind: "newBranch",
      name: "hotfix",
      start: release.oid,
      track: false,
    });
    expect(pickProblem(offer, pick("create", ""), [])).toBe("Enter a name.");
    expect(checkoutRequest(offer, pick("create", "  "))).toBeNull();
  });

  it("has nothing to check out for a tag on a tree", () => {
    const tree = tag("docs", false);
    expect(checkoutOffer(nodeTarget(node({ kind: "tag", tag: tree }), [tree])!, ctx([]), "branches")).toBeNull();
  });

  it("offers a bare commit as a remote branch without a local one: two choices", () => {
    const oid = "d".repeat(40);
    const offer = checkoutOffer({ ref: null, branch: null, oid }, ctx([]), "graph")!;
    expect(offer.source).toBe("commit ddddddd");
    expect(offer.local).toBeNull();
    expect(checkoutRequest(offer, pick("detach"))).toEqual({
      target: { kind: "commit", oid },
      branch: null,
      what: "commit ddddddd",
    });
  });

  it("does not detach again where HEAD is already detached", () => {
    const oid = "d".repeat(40);
    const offer = checkoutOffer({ ref: null, branch: null, oid }, ctx([], { kind: "detached", oid }), "graph")!;
    expect(offer.detach?.blocked).toBe("HEAD is already detached here");
    expect(offer.initial).toBe("create");
    expect(pickProblem(offer, pick("detach"), [])).toBe("HEAD is already detached here");
  });

  it("offers a local branch's label in the graph beside its commit's two choices", () => {
    const local = branch("topic", "local");
    const offer = checkoutOffer(nodeTarget(node({ branch: local, oid: local.oid }), [])!, ctx([local]), "graph")!;
    expect(offer.plain).toBeNull();
    expect(offer.local).toMatchObject({ name: "topic", to: null, label: "Check out local branch 'topic'" });
    expect(offer.initial).toBe("local");
    expect(offer.create).toEqual({ name: "", start: local.oid, remote: null });
  });
});

describe("refActivation", () => {
  it("folds a folder or a group, and does nothing for HEAD", () => {
    expect(refActivation(node({ kind: "folder", children: true }))).toBe("fold");
    expect(refActivation(node({ kind: "group", children: true }))).toBe("fold");
    expect(refActivation(node({ kind: "head" }))).toBeNull();
  });

  it("checks out a branch or a tag, applies a stash, recovers a lost commit", () => {
    expect(refActivation(node({ kind: "local" }))).toBe("checkout");
    expect(refActivation(node({ kind: "remote" }))).toBe("checkout");
    expect(refActivation(node({ kind: "tag" }))).toBe("checkout");
    expect(refActivation(node({ kind: "stash" }))).toBe("apply-stash");
    expect(refActivation(node({ kind: "lost", oid: "a".repeat(40) }))).toBe("recover");
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
