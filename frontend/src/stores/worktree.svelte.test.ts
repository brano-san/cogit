import { describe, expect, it, vi } from "vitest";
import { flushSync } from "svelte";

const ipc = vi.hoisted(() => ({
  worktreeFiles: vi.fn(async () => ({ staged: [], unstaged: [], state: null })),
}));

vi.mock("$lib/ipc", () => ({ ...ipc, toCogitError: (err: unknown) => err }));

const { worktree } = await import("./worktree.svelte");
const { filesView } = await import("./files-view.svelte");

describe("worktree.load called from an effect", () => {
  // App's effect read the Files switches through `load`: ticking "Show directories", which
  // needs no read, read the whole working tree again and greyed out Commit meanwhile.
  it("is not run again by a switch of the Files list", async () => {
    const stop = $effect.root(() => {
      $effect(() => void worktree.load(1 as never));
    });
    flushSync();
    await Promise.resolve();
    filesView.set({ ...filesView.current, directories: !filesView.current.directories });
    flushSync();
    await Promise.resolve();
    expect(ipc.worktreeFiles).toHaveBeenCalledTimes(1);
    stop();
  });
});

describe("a filter searching a clean working tree", () => {
  const row = (path: string, status: string) => ({ path, status }) as never;

  it("reads the unchanged files too, keeps them apart from the changes, and leaves the switch alone", async () => {
    ipc.worktreeFiles.mockClear();
    ipc.worktreeFiles.mockResolvedValueOnce({ staged: [], unstaged: [row("a", "modified"), row("b", "unchanged")], state: null } as never);
    filesView.set({ ...filesView.current, unchanged: false });
    await worktree.searchEveryFile(true, 1 as never);
    expect(ipc.worktreeFiles).toHaveBeenLastCalledWith(1, expect.objectContaining({ unchanged: true }));
    expect(filesView.current.unchanged).toBe(false);
    expect(worktree.unstaged.map((file) => file.path)).toEqual(["a"]);
    expect(worktree.found.map((file) => file.path)).toEqual(["b"]);
    expect(worktree.total).toBe(1);
  });

  it("drops what it found without reading again when it stops", async () => {
    ipc.worktreeFiles.mockClear();
    await worktree.searchEveryFile(false, 1 as never);
    expect(worktree.found).toEqual([]);
    expect(ipc.worktreeFiles).not.toHaveBeenCalled();
    await worktree.load(1 as never);
    expect(ipc.worktreeFiles).toHaveBeenLastCalledWith(1, expect.objectContaining({ unchanged: false }));
  });

  it("is not run again from an effect when the search starts", async () => {
    ipc.worktreeFiles.mockClear();
    const stop = $effect.root(() => {
      $effect(() => void worktree.load(1 as never));
    });
    flushSync();
    void worktree.searchEveryFile(true, 1 as never);
    flushSync();
    await Promise.resolve();
    expect(ipc.worktreeFiles).toHaveBeenCalledTimes(2);
    await worktree.searchEveryFile(false, 1 as never);
    stop();
  });
});
