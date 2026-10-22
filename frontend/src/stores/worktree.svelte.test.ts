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

describe("worktree.load while a filter searches a clean working tree", () => {
  it("asks for the unchanged files too, the Unchanged switch left as it is", async () => {
    ipc.worktreeFiles.mockClear();
    filesView.set({ ...filesView.current, unchanged: false });
    worktree.everyFile = true;
    await worktree.load(1 as never);
    expect(ipc.worktreeFiles).toHaveBeenLastCalledWith(1, expect.objectContaining({ unchanged: true }));
    expect(filesView.current.unchanged).toBe(false);
    worktree.everyFile = false;
    await worktree.load(1 as never);
    expect(ipc.worktreeFiles).toHaveBeenLastCalledWith(1, expect.objectContaining({ unchanged: false }));
  });

  it("is not run again from an effect when the search starts", async () => {
    ipc.worktreeFiles.mockClear();
    const stop = $effect.root(() => {
      $effect(() => void worktree.load(1 as never));
    });
    flushSync();
    worktree.everyFile = true;
    flushSync();
    await Promise.resolve();
    expect(ipc.worktreeFiles).toHaveBeenCalledTimes(1);
    worktree.everyFile = false;
    stop();
  });
});
