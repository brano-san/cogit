import { describe, expect, it } from "vitest";
import { STASH_BUTTONS, STASH_MODES, stashNameProblem, stashRequest } from "./stash-modes";

describe("the Stash dialog", () => {
  it("offers Stash All, + Keep Index and + Keep Working Tree, in that order", () => {
    expect(STASH_MODES.map((entry) => entry.label)).toEqual([
      "Stash All",
      "+ Keep Index",
      "+ Keep Working Tree",
    ]);
  });

  // Stash All stood leftmost of the actions with Cancel at the right edge (R-167).
  it("puts the primary Stash All rightmost, after the variants", () => {
    expect(STASH_BUTTONS.map((entry) => entry.label)).toEqual([
      "+ Keep Index",
      "+ Keep Working Tree",
      "Stash All",
    ]);
  });

  it("asks for a name and says so", () => {
    expect(stashNameProblem("   ")).toBe("Enter a name.");
    expect(stashNameProblem("parser, half done")).toBeNull();
  });
});

describe("stashRequest", () => {
  it("stashes everything, untracked files included, for Stash All", () => {
    expect(stashRequest({ mode: "all", message: " wip " })).toEqual({
      kind: "push",
      message: "wip",
      includeUntracked: true,
      keepIndex: false,
    });
  });

  it("keeps the index for + Keep Index", () => {
    expect(stashRequest({ mode: "keepIndex", message: "wip" })).toMatchObject({
      kind: "push",
      keepIndex: true,
    });
  });

  it("goes through stash create and store for + Keep Working Tree", () => {
    expect(stashRequest({ mode: "keepWorktree", message: "wip" })).toEqual({
      kind: "keepWorktree",
      message: "wip",
    });
  });
});
