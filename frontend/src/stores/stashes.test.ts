import { beforeEach, describe, expect, it, vi } from "vitest";

const ipc = vi.hoisted(() => ({
  stashApply: vi.fn(async () => null),
  listStashes: vi.fn(async () => []),
}));

vi.mock("$lib/ipc", () => ipc);

const { stashes } = await import("./stashes.svelte");

const A = 1 as never;

beforeEach(() => {
  ipc.stashApply.mockClear();
  stashes.clear();
});

// Item 40: the Apply Stash dialog's Apply & Drop and Restore Index reach git.
describe("applying a stash", () => {
  it("hands Apply & Drop and Restore Index to the backend", async () => {
    await stashes.apply(A, 2, true, true);

    expect(ipc.stashApply).toHaveBeenCalledWith(A, 2, true, true);
  });

  it("leaves the index as git applies it unless asked", async () => {
    await stashes.apply(A, 0, false);

    expect(ipc.stashApply).toHaveBeenCalledWith(A, 0, false, false);
  });
});
