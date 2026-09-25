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

// A pre-commit hook's refusal, or a failed stage, came out as "Could not read the working
// tree", and the step counted as done; a failed read after a real commit counted as a
// failed commit, so the box kept the message and a retry committed twice.
describe("a write and the read after it", () => {
  it("hands a refused write to the caller and keeps the read error clear", async () => {
    ipc.stagePaths.mockRejectedValueOnce(new Error("index.lock exists"));

    await expect(worktree.stage(REPO, ["a.txt"])).rejects.toThrow("index.lock exists");

    expect(worktree.error).toBeNull();
    expect(ipc.worktreeFiles).not.toHaveBeenCalled();
  });

  it("does not call a commit failed when only the read after it failed", async () => {
    ipc.worktreeFiles.mockRejectedValueOnce(new Error("cannot read"));

    await expect(worktree.commit(REPO, "message", false, false)).resolves.toBeUndefined();

    expect(worktree.error).not.toBeNull();
  });
});
