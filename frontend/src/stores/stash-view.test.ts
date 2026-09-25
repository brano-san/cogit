import { beforeEach, describe, expect, it, vi } from "vitest";

const contents = (rev: string) => ({
  worktree: [],
  index: [],
  untracked: [],
  base: "base",
  worktreeRev: rev,
  indexRev: `${rev}^2`,
  untrackedRev: null,
});

vi.mock("$lib/ipc", () => ({
  stashContents: vi.fn(async () => contents("s1")),
  toCogitError: (err: unknown) => err,
}));

const { stashView } = await import("./stash-view.svelte");
const entry = (oid: string, index = 0) => ({ index, oid, message: "", timestamp: 0 });

beforeEach(() => stashView.clear());

// Drop of the stash open in Files left Files and Diff on it, and the toolbar's Stage and
// Discard off, until Working Tree was clicked.
describe("the stash Files shows, after the stash list changed", () => {
  it("goes once it is no longer in the list", async () => {
    await stashView.select(1 as never, 0);

    expect(stashView.forgetIfGone([entry("s2")])).toBe(true);

    expect(stashView.contents).toBeNull();
  });

  it("stays while it is, whatever its index is now", async () => {
    await stashView.select(1 as never, 0);

    expect(stashView.forgetIfGone([entry("s0", 0), entry("s1", 1)])).toBe(false);

    expect(stashView.contents?.worktreeRev).toBe("s1");
  });
});
