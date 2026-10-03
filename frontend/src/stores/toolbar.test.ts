import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RepoId } from "$lib/ipc";

const isMergedIntoHead = vi.fn(async (_repo: RepoId, _commit: string) => true);
vi.mock("$lib/ipc", () => ({ isMergedIntoHead }));
vi.mock("$lib/settings-file", () => ({ readKey: vi.fn(), writeKey: vi.fn() }));

const { toolbar } = await import("./toolbar.svelte");

const REPO = 1 as RepoId;

describe("toolbar.checkMerged", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    isMergedIntoHead.mockClear();
  });
  afterEach(() => vi.useRealTimers());

  it("sends one question for a run of selections and answers for the last", async () => {
    for (const commit of ["a", "b", "c", "d", "e"]) {
      void toolbar.checkMerged(REPO, commit, "head");
      await vi.advanceTimersByTimeAsync(20);
    }
    expect(isMergedIntoHead).not.toHaveBeenCalled();
    expect(toolbar.merged).toBeUndefined();

    await vi.advanceTimersByTimeAsync(200);
    expect(isMergedIntoHead).toHaveBeenCalledTimes(1);
    expect(isMergedIntoHead).toHaveBeenCalledWith(REPO, "e");
    expect(toolbar.merged).toBe(true);
  });

  it("asks nothing for the commit at HEAD", async () => {
    void toolbar.checkMerged(REPO, "head", "head");
    await vi.advanceTimersByTimeAsync(200);
    expect(isMergedIntoHead).not.toHaveBeenCalled();
    expect(toolbar.merged).toBeUndefined();
  });
});
