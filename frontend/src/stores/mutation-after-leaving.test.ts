import { beforeEach, describe, expect, it, vi } from "vitest";

// Writes answer only when the test says so; reads answer at once with the repository they
// were asked about, so a test can see which repository a store read back after a write.
const held = vi.hoisted(() => new Map<string, () => void>());
const reads = vi.hoisted(() => [] as string[]);

vi.mock("$lib/ipc", () => {
  const later =
    (name: string) =>
    (repo: unknown) =>
      new Promise<void>((resolve) => held.set(`${name}:${String(repo)}`, resolve));
  const read =
    <T>(name: string, value: (repo: unknown) => T) =>
    async (repo: unknown) => {
      reads.push(`${name}:${String(repo)}`);
      return value(repo);
    };
  return {
    CogitError: class extends Error {},
    stashApply: vi.fn(later("apply")),
    listStashes: vi.fn(read("stashes", (repo) => [{ index: 0, message: `of ${String(repo)}` }])),
    removeWorktree: vi.fn(later("remove")),
    listWorktrees: vi.fn(read("worktrees", (repo) => [{ path: `of ${String(repo)}` }])),
    stagePaths: vi.fn(later("stage")),
    worktreeFiles: vi.fn(
      read("files", (repo) => ({ staged: [{ path: `of ${String(repo)}` }], unstaged: [] })),
    ),
    flowStart: vi.fn(later("start")),
    flowStatus: vi.fn(
      read("flow", (repo) => ({ initialised: true, config: {}, branches: [{ name: `of ${String(repo)}` }] })),
    ),
    resolveConflict: vi.fn(later("take")),
    conflictedPaths: vi.fn(read("conflicts", (repo) => [`of ${String(repo)}`])),
    conflictText: vi.fn(async () => ({ base: "", ours: "", theirs: "" })),
    mergePreview: vi.fn(async () => []),
  };
});

const { stashes } = await import("./stashes.svelte");
const { worktrees } = await import("./worktrees.svelte");
const { worktree } = await import("./worktree.svelte");
const { flow } = await import("./flow.svelte");
const { conflicts } = await import("./conflicts.svelte");

const A = 1 as never;
const B = 2 as never;

function answer(key: string): void {
  const resolve = held.get(key);
  if (!resolve) throw new Error(`nothing asked for ${key}; asked: ${[...held.keys()].join(", ")}`);
  held.delete(key);
  resolve();
}

beforeEach(() => {
  held.clear();
  for (const store of [stashes, worktrees, worktree, flow, conflicts]) store.clear();
});

// A write finishes after the panels have moved to B. The store read A back on its own and
// wrote the answer over B's panel: A's stashes offered for Drop in B, A's staged files in
// B's Files, the worktree commands sent to A.
describe("a write that finishes after the panels have left its repository", () => {
  it("does not put A's stashes in B's list", async () => {
    const write = stashes.apply(A, 0, false);
    stashes.clear();
    await stashes.refresh(B);
    reads.length = 0;
    answer("apply:1");
    await write;
    expect(reads).toEqual([]);
    expect(stashes.entries.map((entry) => entry.message)).toEqual(["of 2"]);
  });

  it("does not point the worktree panel back at A", async () => {
    await worktrees.refresh(A);
    const write = worktrees.remove("x", false);
    worktrees.clear();
    await worktrees.refresh(B);
    reads.length = 0;
    answer("remove:1");
    await write;
    expect(reads).toEqual([]);
    expect(worktrees.repo).toBe(B);
    expect(worktrees.entries.map((entry) => entry.path)).toEqual(["of 2"]);
  });

  it("does not put A's staged files in B's Files", async () => {
    const write = worktree.stage(A, ["a.txt"]);
    worktree.clear();
    await worktree.load(B);
    reads.length = 0;
    answer("stage:1");
    await write;
    expect(reads).toEqual([]);
    expect(worktree.staged.map((entry) => entry.path)).toEqual(["of 2"]);
  });

  it("does not put A's flow branches in B's panel", async () => {
    const write = flow.start(A, "feature", "x");
    flow.clear();
    await flow.refresh(B);
    reads.length = 0;
    answer("start:1");
    await write;
    expect(reads).toEqual([]);
    expect(flow.status.branches.map((branch) => branch.name)).toEqual(["of 2"]);
  });

  it("does not put A's conflicted files in B's list", async () => {
    await conflicts.open(A, "a.txt");
    const write = conflicts.take(A, "ours");
    conflicts.clear();
    await conflicts.refresh(B);
    reads.length = 0;
    answer("take:1");
    await write;
    expect(reads).toEqual([]);
    expect(conflicts.paths).toEqual(["of 2"]);
  });
});

describe("a write that finishes in the repository still shown", () => {
  it("reads the list back", async () => {
    const write = stashes.apply(A, 0, false);
    reads.length = 0;
    answer("apply:1");
    await write;
    expect(reads).toEqual(["stashes:1"]);
  });
});
