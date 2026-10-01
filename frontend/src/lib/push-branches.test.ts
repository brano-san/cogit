import { describe, expect, it } from "vitest";
import { branchPushes, pushBranchesTitle, startRemote } from "./push-branches";

const remotes = ["origin", "up"];

describe("branchPushes", () => {
  it("sends each branch to its tracked branch or its own name", () => {
    const pushes = branchPushes(
      [
        { name: "a", upstream: "origin/alpha" },
        { name: "b", upstream: null },
      ],
      "origin",
      remotes,
      true,
    );
    expect(pushes.map((push) => push.refspec)).toEqual([
      "refs/heads/a:refs/heads/alpha",
      "refs/heads/b:refs/heads/b",
    ]);
  });

  it("starts tracking only for branches that track nothing, and only when asked", () => {
    const branches = [
      { name: "a", upstream: "origin/a" },
      { name: "b", upstream: null },
    ];
    expect(branchPushes(branches, "origin", remotes, true).map((push) => push.track)).toEqual([false, true]);
    expect(branchPushes(branches, "origin", remotes, false).map((push) => push.track)).toEqual([false, false]);
  });

  it("uses the chosen remote's own name for a branch tracked on another remote", () => {
    const [push] = branchPushes([{ name: "a", upstream: "up/x" }], "origin", remotes, false);
    expect(push?.refspec).toBe("refs/heads/a:refs/heads/a");
  });
});

describe("startRemote and title", () => {
  it("opens on the first branch's remote, else the primary", () => {
    expect(startRemote([{ name: "a", upstream: "up/a" }], remotes, "origin")).toBe("up");
    expect(startRemote([{ name: "a", upstream: null }], remotes, "origin")).toBe("origin");
    expect(startRemote([], [], null)).toBeNull();
  });

  it("counts the branches", () => {
    expect(pushBranchesTitle(1, "origin")).toBe("Push 1 branch to remote 'origin'");
    expect(pushBranchesTitle(3, "origin")).toBe("Push 3 branches to remote 'origin'");
  });
});
