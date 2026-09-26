import { describe, expect, it } from "vitest";
import type { RepoId } from "$lib/ipc";
import { reopenClick, repoClick, type RepoClickState } from "./repo-click";

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
