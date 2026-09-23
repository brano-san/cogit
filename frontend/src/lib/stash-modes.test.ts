import { describe, expect, it } from "vitest";
import { STASH_MODES, stashNameProblem, stashRequest } from "./stash-modes";

describe("the Stash dialog", () => {
  it("offers Stash All, + Keep Index and + Keep Working Tree, in that order", () => {
    expect(STASH_MODES.map((entry) => entry.label)).toEqual([
      "Stash All",
      "+ Keep Index",
      "+ Keep Working Tree",
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
