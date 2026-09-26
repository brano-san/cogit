import { describe, expect, it } from "vitest";
import type { RepoId } from "$lib/ipc";
import { closeStep, holdsPanels, reopenClick, repoClick, type RepoClickState } from "./repo-click";

const id = (n: number) => n as unknown as RepoId;
const alpha = { repo: id(1), root: "E:/w/alpha" };
const beta = { repo: id(2), root: "E:/w/beta" };

function state(over: Partial<RepoClickState> = {}): RepoClickState {
  return { shown: id(1), opening: null, moduleOwner: null, worktreeOwner: null, ...over };
}

describe("repoClick", () => {
  it("reloads nothing when the repository on screen is clicked again", () => {
    expect(repoClick(alpha, state())).toBe("stay");
  });

  it("does not start a second open of a repository already opening", () => {
    expect(repoClick(beta, state({ opening: "E:/w/beta" }))).toBe("stay");
  });

  it("switches back to the owner of the submodule on screen", () => {
    expect(repoClick(alpha, state({ shown: id(7), moduleOwner: id(1) }))).toBe("return");
  });

  it("switches back to the owner of the worktree on screen", () => {
    expect(repoClick(alpha, state({ shown: id(8), worktreeOwner: "E:\\w\\alpha"}))).toBe(
      "return",
    );
  });

  it("opens any other repository in full", () => {
    expect(repoClick(beta, state())).toBe("open");
    expect(repoClick(beta, state({ shown: id(7), moduleOwner: id(1) }))).toBe("open");
  });

  it("opens when nothing is on screen yet", () => {
    expect(repoClick(alpha, state({ shown: null }))).toBe("open");
  });
});

// The two clicks of a double-click on a closed row opened it twice, the second overtaking
// the first.
describe("reopenClick", () => {
  it("does not open a closed row again while its open is under way", () => {
    expect(reopenClick("E:/w/alpha", "E:\\w\\alpha")).toBe("stay");
  });

  it("opens a closed row otherwise", () => {
    expect(reopenClick("E:/w/alpha", null)).toBe("open");
    expect(reopenClick("E:/w/alpha", "E:/w/beta")).toBe("open");
  });
});

// Close Repository on the owner of the submodule on screen left the panels on the submodule
// and the tree pointing at the closed repository.
describe("holdsPanels", () => {
  it("counts the owner of the submodule or worktree on screen as the one shown", () => {
    expect(holdsPanels(alpha, state())).toBe(true);
    expect(holdsPanels(alpha, state({ shown: id(7), moduleOwner: id(1) }))).toBe(true);
    expect(holdsPanels(alpha, state({ shown: id(8), worktreeOwner: "E:/w/alpha" }))).toBe(true);
  });

  it("leaves any other row to itself", () => {
    expect(holdsPanels(beta, state({ shown: id(7), moduleOwner: id(1) }))).toBe(false);
  });
});

// Ctrl+W and Close Repository carry one chord and did different things: Ctrl+W left the
// start screen with other repositories open, and on a submodule or worktree closed its
// registration and left the panels pointing at it.
describe("closeStep", () => {
  it("goes back to the owner of the worktree or submodule on screen", () => {
    expect(closeStep(state({ shown: id(8), worktreeOwner: "E:/w/alpha" }))).toEqual({
      kind: "worktree",
      owner: "E:/w/alpha",
    });
    expect(closeStep(state({ shown: id(7), moduleOwner: id(1) }))).toEqual({ kind: "module" });
  });

  it("closes the listed repository on screen, and nothing when none is", () => {
    expect(closeStep(state())).toEqual({ kind: "repository", repo: id(1) });
    expect(closeStep(state({ shown: null }))).toEqual({ kind: "none" });
  });
});
