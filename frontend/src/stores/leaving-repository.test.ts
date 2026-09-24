import { beforeEach, describe, expect, it, vi } from "vitest";

// Every read answers only when the test says so, keyed by command and repository, so a
// test can hold repository A's answer until the panels have moved on to B.
const held = vi.hoisted(() => new Map<string, (value: unknown) => void>());

vi.mock("$lib/ipc", () => {
  const later =
    (name: string) =>
    (repo: unknown, ...rest: unknown[]) =>
      new Promise((resolve) => held.set([name, repo, ...rest].join(":"), resolve));
  return {
    listStashes: vi.fn(later("stashes")),
    lostCommits: vi.fn(later("lost")),
    listRemotes: vi.fn(later("remotes")),
    remoteUrl: vi.fn(later("url")),
    hasToken: vi.fn(async () => false),
    flowStatus: vi.fn(later("flow")),
    conflictedPaths: vi.fn(later("conflicts")),
    listWorktrees: vi.fn(later("worktrees")),
    listSubmodules: vi.fn(later("submodules")),
    refDates: vi.fn(later("dates")),
  };
});

const { stashes } = await import("./stashes.svelte");
const { recovery } = await import("./recovery.svelte");
const { network } = await import("./network.svelte");
const { flow } = await import("./flow.svelte");
const { conflicts } = await import("./conflicts.svelte");
const { worktrees } = await import("./worktrees.svelte");
const { submodules } = await import("./submodules.svelte");
const { refs } = await import("./refs.svelte");

const A = 1 as never;
const B = 2 as never;

function answer(key: string, value: unknown): void {
  const resolve = held.get(key);
  if (!resolve) throw new Error(`nothing asked for ${key}; asked: ${[...held.keys()].join(", ")}`);
  held.delete(key);
  resolve(value);
}

const flushed = () => new Promise((resolve) => setTimeout(resolve, 0));

beforeEach(() => {
  held.clear();
  for (const store of [stashes, recovery, network, flow, conflicts, worktrees, submodules, refs]) {
    store.clear();
  }
});

// The panels switch repository by clearing every store and reading B. An answer for A
// that arrives after that used to be written over B's panels: A's stashes offered for
// Apply in B, A's worktrees removed through B.
describe("an answer for the repository the panels have left", () => {
  it("does not reach the stash list", async () => {
    const late = stashes.refresh(A);
    stashes.clear();
    answer("stashes:1", [{ index: 0, message: "from A" }]);
    await late;
    expect(stashes.entries).toEqual([]);
  });

  it("does not reach the lost commits", async () => {
    const late = recovery.refresh(A);
    recovery.clear();
    answer("lost:1", [{ oid: "a" }]);
    await late;
    expect(recovery.lost).toEqual([]);
  });

  it("does not reach the remotes", async () => {
    const late = network.refresh(A);
    network.clear();
    answer("remotes:1", ["origin"]);
    await late;
    expect(network.remotes).toEqual([]);
    expect(held.size).toBe(0);
  });

  it("does not reach the flow status", async () => {
    const late = flow.refresh(A);
    flow.clear();
    answer("flow:1", { initialised: true, config: {}, branches: [] });
    await late;
    expect(flow.status.initialised).toBe(false);
  });

  it("does not reach the conflicted paths", async () => {
    const late = conflicts.refresh(A);
    conflicts.clear();
    answer("conflicts:1", ["a.txt"]);
    await late;
    expect(conflicts.paths).toEqual([]);
  });

  it("does not reach the worktree list", async () => {
    const late = worktrees.refresh(A);
    worktrees.clear();
    answer("worktrees:1", [{ path: "C:/a-worktree" }]);
    await late;
    expect(worktrees.entries).toEqual([]);
  });

  it("does not reach the next owner's submodule tree", async () => {
    const late = submodules.own(A, "C:/a");
    const next = submodules.own(B, "C:/b");
    answer("submodules:2:", [{ path: "b-module" }]);
    await next;
    answer("submodules:1:", [{ path: "a-module" }]);
    await late;
    expect(submodules.top).toEqual([{ path: "b-module" }]);
  });

  it("does not reach the ref dates", async () => {
    const late = refs.loadDates(A);
    refs.clear();
    answer("dates:1", [{ fullName: "refs/heads/a", timestamp: 1 }]);
    await late;
    expect(refs.dates.size).toBe(0);
  });
});

describe("two reads of the same list", () => {
  it("keep the newer answer when the older one arrives last", async () => {
    const older = stashes.refresh(A);
    const newer = stashes.refresh(B);
    answer("stashes:2", [{ index: 0, message: "from B" }]);
    await newer;
    answer("stashes:1", [{ index: 0, message: "from A" }]);
    await older;
    await flushed();
    expect(stashes.entries).toEqual([{ index: 0, message: "from B" }]);
  });
});

// A re-read after a mutation wrote the whole tree from what it read before its await: a
// node opened meanwhile lost its children, and of two re-reads the one to finish last won.
describe("re-reading the submodule tree", () => {
  it("keeps a node opened while the read was on its way", async () => {
    const owning = submodules.own(A, "C:/a");
    answer("submodules:1:", [{ path: "k" }]);
    await owning;

    const refresh = submodules.refresh();
    const opening = submodules.toggle({ key: "k" } as never);
    answer("submodules:1:k", [{ path: "k/inner" }]);
    await opening;
    answer("submodules:1:", [{ path: "k" }]);
    await refresh;

    expect(submodules.expanded.has("k")).toBe(true);
    expect(submodules.children.get("k")).toEqual([{ path: "k/inner" }]);
  });

  it("keeps the newest of two re-reads", async () => {
    const owning = submodules.own(A, "C:/untouched");
    answer("submodules:1:", [{ path: "k" }]);
    await owning;

    const older = submodules.refresh();
    const olderAnswer = held.get("submodules:1:")!;
    held.delete("submodules:1:");
    const newer = submodules.refresh();
    answer("submodules:1:", [{ path: "k" }, { path: "new" }]);
    await newer;
    olderAnswer([{ path: "k" }]);
    await older;

    expect(submodules.top).toEqual([{ path: "k" }, { path: "new" }]);
  });
});
