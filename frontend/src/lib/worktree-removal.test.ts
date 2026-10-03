import { describe, expect, it, vi } from "vitest";
import type { WorktreeEntry } from "./ipc";
import { removeWorktree } from "./worktree-removal";

const TARGET: WorktreeEntry = {
  path: "D:/src/wt",
  name: "wt",
  branch: "topic",
  head: "abc",
  isMain: false,
  isCurrent: false,
  locked: null,
  missing: false,
  dirty: false,
  changed: 0,
  untracked: 0,
  bare: false,
  hasSubmodules: false,
};

/** `fails`: whether each removal call fails, in order. Every question is answered yes. */
function host(fails: boolean[], listed: boolean, leftover = false) {
  return {
    remove: vi.fn(async () => {
      if (fails.shift()) throw new Error("boom");
    }),
    fail: vi.fn(),
    refreshSafety: vi.fn(async () => {}),
    listed: vi.fn(() => listed),
    leftover: vi.fn(async () => leftover),
    deleteLeftover: vi.fn(async () => {}),
    ask: vi.fn(async () => true),
  };
}

describe("removeWorktree", () => {
  it("asks nothing after a removal that worked", async () => {
    const h = host([false], false);
    await removeWorktree(TARGET, false, h);
    expect(h.remove).toHaveBeenCalledWith("D:/src/wt", false);
    expect(h.refreshSafety).toHaveBeenCalledOnce();
    expect(h.ask).not.toHaveBeenCalled();
    expect(h.fail).not.toHaveBeenCalled();
  });

  it("offers --force while the worktree is still registered, and retries with it", async () => {
    const h = host([true, false], true);
    await removeWorktree(TARGET, false, h);
    expect(h.fail).toHaveBeenCalledWith(expect.any(Error), "Could not remove the worktree");
    expect(h.ask).toHaveBeenCalledOnce();
    expect(h.ask).toHaveBeenCalledWith(expect.objectContaining({ confirm: "Force Remove" }));
    expect(h.remove).toHaveBeenLastCalledWith("D:/src/wt", true);
    expect(h.refreshSafety).toHaveBeenCalledTimes(2);
  });

  it("asks nothing more when the forced removal fails too", async () => {
    const h = host([true], true);
    await removeWorktree(TARGET, true, h);
    expect(h.ask).not.toHaveBeenCalled();
    expect(h.leftover).not.toHaveBeenCalled();
  });

  it("offers to delete the folder git left behind after dropping the registration", async () => {
    const h = host([true], false, true);
    await removeWorktree(TARGET, false, h);
    expect(h.ask).toHaveBeenCalledWith(expect.objectContaining({ confirm: "Delete Folder", items: ["D:/src/wt"] }));
    expect(h.deleteLeftover).toHaveBeenCalledWith("D:/src/wt");
  });

  it("does nothing when nothing is left on disk", async () => {
    const h = host([true], false, false);
    await removeWorktree(TARGET, false, h);
    expect(h.ask).not.toHaveBeenCalled();
    expect(h.deleteLeftover).not.toHaveBeenCalled();
  });
});
