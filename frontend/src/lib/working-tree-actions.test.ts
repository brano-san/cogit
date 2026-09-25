import { describe, expect, it, vi } from "vitest";
import type { RepoId } from "$lib/ipc";
import { runWorkingTreeAction, type WorkingTreeHost } from "./working-tree-actions";

const REPO = 7 as unknown as RepoId;

const files = {
  staged: [{ path: "s.txt" }],
  unstaged: [
    { path: "a.txt", status: "modified" },
    { path: "new.txt", status: "untracked" },
  ],
};

function host(answer = true) {
  const writes: string[] = [];
  const done: WorkingTreeHost = {
    stage: vi.fn(async () => void writes.push("stage")),
    unstage: vi.fn(async () => void writes.push("unstage")),
    discard: vi.fn(async () => void writes.push("discard")),
    mutate: vi.fn(async (step: (repo: RepoId) => Promise<unknown>, _paths: string[], _readsBack: boolean) => {
      await step(REPO);
      return true;
    }),
    confirmDiscard: vi.fn(async () => answer),
    focusCommit: vi.fn(async () => {}),
  };
  return { host: done, writes };
}

// The Working Tree row's Stage, Unstage and Discard refreshed with no paths: the Diff kept
// showing the discarded hunks, and Stage lines then worked on lines no longer there.
describe("runWorkingTreeAction", () => {
  it("names the files it stages, so the diff of one of them is read again", async () => {
    const { host: h, writes } = host();
    await runWorkingTreeAction("wt-stage", files, h);
    expect(h.mutate).toHaveBeenCalledWith(expect.any(Function), ["a.txt", "new.txt"], true);
    expect(h.stage).toHaveBeenCalledWith(REPO, ["a.txt", "new.txt"]);
    expect(writes).toEqual(["stage"]);
  });

  it("names the files it unstages", async () => {
    const { host: h } = host();
    await runWorkingTreeAction("wt-unstage", files, h);
    expect(h.mutate).toHaveBeenCalledWith(expect.any(Function), ["s.txt"], true);
    expect(h.unstage).toHaveBeenCalledWith(REPO, ["s.txt"]);
  });

  it("asks, then discards the tracked changes and names them", async () => {
    const { host: h } = host();
    await runWorkingTreeAction("wt-discard", files, h);
    expect(h.confirmDiscard).toHaveBeenCalledWith(["a.txt"]);
    expect(h.mutate).toHaveBeenCalledWith(expect.any(Function), ["a.txt"], true);
    expect(h.discard).toHaveBeenCalledWith(REPO, ["a.txt"]);
  });

  it("discards nothing when the answer is no", async () => {
    const { host: h } = host(false);
    await runWorkingTreeAction("wt-discard", files, h);
    expect(h.mutate).not.toHaveBeenCalled();
  });

  // With the Commit Message panel hidden there was no field to focus, and nothing happened.
  it("takes Commit to the commit box, which shows its panel", async () => {
    const { host: h } = host();
    await runWorkingTreeAction("wt-commit", files, h);
    expect(h.focusCommit).toHaveBeenCalled();
    expect(h.mutate).not.toHaveBeenCalled();
  });
});
