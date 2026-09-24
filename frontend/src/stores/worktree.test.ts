import { beforeEach, describe, expect, it, vi } from "vitest";

const ipc = vi.hoisted(() => ({
  stageAll: vi.fn(async () => {}),
  stagePaths: vi.fn(async () => {}),
  worktreeFiles: vi.fn(async () => ({
    staged: [],
    unstaged: [
      { path: "a.txt", oldPath: null, status: "modified", mode: "plain", modeChange: null, similarity: null },
      { path: "b.txt", oldPath: null, status: "untracked", mode: "plain", modeChange: null, similarity: null },
    ],
  })),
}));

vi.mock("$lib/ipc", () => ({
  ...ipc,
  createCommit: vi.fn(async () => ""),
  discardPaths: vi.fn(async () => {}),
  unstagePaths: vi.fn(async () => {}),
  toCogitError: (err: unknown) => err,
}));

const { worktree } = await import("./worktree.svelte");
const REPO = 1 as never;

beforeEach(async () => {
  worktree.clear();
  await worktree.load(REPO);
  vi.clearAllMocks();
});

describe("staging from the Unstaged list", () => {
  it("stages every row with one git add --all, no path list", async () => {
    await worktree.stage(REPO, ["b.txt", "a.txt"]);

    expect(ipc.stageAll).toHaveBeenCalledWith(REPO, 2);
    expect(ipc.stagePaths).not.toHaveBeenCalled();
  });

  it("stages a part by its paths", async () => {
    await worktree.stage(REPO, ["a.txt"]);

    expect(ipc.stagePaths).toHaveBeenCalledWith(REPO, ["a.txt"]);
    expect(ipc.stageAll).not.toHaveBeenCalled();
  });

  it("reads the list back either way", async () => {
    await worktree.stage(REPO, ["a.txt", "b.txt"]);

    expect(ipc.worktreeFiles).toHaveBeenCalledTimes(1);
  });
});
