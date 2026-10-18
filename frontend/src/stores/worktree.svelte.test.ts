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
