import { beforeEach, describe, expect, it, vi } from "vitest";

const commands = { commitTreeFiles: vi.fn() };

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands }));

const { commitTree } = await import("./commit-tree.svelte");

const REPO = 1 as unknown as import("$lib/ipc").RepoId;

function answer(paths: string[]) {
  return { status: "ok" as const, data: paths };
}

describe("commit tree store", () => {
  beforeEach(() => {
    commands.commitTreeFiles.mockReset();
    commitTree.clear();
  });

  it("reads a commit's tree once", async () => {
    commands.commitTreeFiles.mockResolvedValue(answer(["a", "b"]));

    await commitTree.load(REPO, "c1");
    await commitTree.load(REPO, "c1");

    expect(commitTree.paths).toEqual(["a", "b"]);
    expect(commands.commitTreeFiles).toHaveBeenCalledOnce();
  });

  it("drops an answer for a commit that is no longer selected", async () => {
    let finish: (value: unknown) => void = () => {};
    commands.commitTreeFiles.mockReturnValueOnce(new Promise((resolve) => (finish = resolve)));
    commands.commitTreeFiles.mockResolvedValueOnce(answer(["new"]));

    const slow = commitTree.load(REPO, "old");
    await commitTree.load(REPO, "new");
    finish(answer(["old"]));
    await slow;

    expect(commitTree.oid).toBe("new");
    expect(commitTree.paths).toEqual(["new"]);
  });

  it("lists nothing extra when the tree cannot be read", async () => {
    commands.commitTreeFiles.mockResolvedValue({ status: "error", error: { kind: "internal", data: "x" } });

    await commitTree.load(REPO, "c1");

    expect(commitTree.paths).toEqual([]);
  });

  // The failure was kept as an empty tree: turning Unchanged off and on read nothing again,
  // and nobody was told.
  it("reads a tree that failed again, and says it failed", async () => {
    commands.commitTreeFiles.mockResolvedValueOnce({ status: "error", error: { kind: "internal", data: "x" } });
    commands.commitTreeFiles.mockResolvedValueOnce(answer(["a"]));

    await commitTree.load(REPO, "c1");
    expect(commitTree.error).not.toBeNull();
    await commitTree.load(REPO, "c1");

    expect(commands.commitTreeFiles).toHaveBeenCalledTimes(2);
    expect(commitTree.paths).toEqual(["a"]);
    expect(commitTree.error).toBeNull();
  });
});
