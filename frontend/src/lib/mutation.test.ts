import { describe, expect, it, vi } from "vitest";
import { runMutation, type MutationContext } from "./mutation";

const REPO = 7 as never;

function context(overrides: Partial<MutationContext> = {}) {
  const calls: string[] = [];
  let epoch = 0;
  const base: MutationContext = {
    repo: () => REPO,
    epoch: () => epoch,
    report: vi.fn(() => calls.push("report")),
    loadWorktree: vi.fn(async () => void calls.push("worktree_files")),
    after: vi.fn(async () => void calls.push("after")),
  };
  return { calls, leave: () => (epoch += 1), context: { ...base, ...overrides } };
}

describe("runMutation", () => {
  it("reads the working tree back, then refreshes the rest", async () => {
    const { calls, context: c } = context();

    expect(await runMutation(c, async () => void calls.push("write"), ["a"], false)).toBe(true);

    expect(calls).toEqual(["write", "worktree_files", "after"]);
    expect(c.after).toHaveBeenCalledWith(["a"]);
  });

  // Staging read the list twice: once in the worktree store, once here.
  it("does not read the list a second time after a step that read it back", async () => {
    const { calls, context: c } = context();

    await runMutation(c, async () => void calls.push("write + worktree_files"), ["a"], true);

    expect(calls).toEqual(["write + worktree_files", "after"]);
  });

  it("reports a failed step and reads nothing", async () => {
    const { calls, context: c } = context();
    const failure = new Error("git said no");

    const done = await runMutation(c, () => Promise.reject(failure), [], false);

    expect(done).toBe(false);
    expect(c.report).toHaveBeenCalledWith(failure);
    expect(calls).toEqual(["report"]);
  });

  it("reads nothing into panels that moved to another repository meanwhile", async () => {
    const { calls, leave, context: c } = context();

    await runMutation(c, async () => void leave(), [], false);

    expect(calls).toEqual([]);
  });

  it("does nothing without a repository", async () => {
    const step = vi.fn(async () => {});
    const { context: c } = context({ repo: () => null });

    expect(await runMutation(c, step, [], false)).toBe(false);
    expect(step).not.toHaveBeenCalled();
  });
});
