import { describe, expect, it } from "vitest";
import { dropActions, parseDrag, serialiseDrag } from "./drop-target";

describe("serialiseDrag and parseDrag", () => {
  it("round-trips a branch", () => {
    expect(parseDrag(serialiseDrag({ kind: "branch", id: "feature" }))).toEqual({
      kind: "branch",
      id: "feature",
    });
  });

  it("round-trips a commit", () => {
    expect(parseDrag(serialiseDrag({ kind: "commit", id: "abc123" }))).toEqual({
      kind: "commit",
      id: "abc123",
    });
  });

  it("returns null for anything else the browser hands over", () => {
    expect(parseDrag("some text a user dragged in")).toBeNull();
    expect(parseDrag("")).toBeNull();
    expect(parseDrag('{"kind":"nonsense","id":"x"}')).toBeNull();
  });

  it("returns null for a payload missing an id", () => {
    expect(parseDrag('{"kind":"branch"}')).toBeNull();
  });
});

describe("dropActions", () => {
  const branch = { kind: "branch", id: "feature" } as const;
  const other = { kind: "branch", id: "main" } as const;
  const commit = { kind: "commit", id: "abc" } as const;

  it("offers merge and rebase when a branch lands on a branch", () => {
    const ids = dropActions(branch, other, false).map((a) => a.id);
    expect(ids).toContain("merge");
    expect(ids).toContain("rebase");
  });

  it("offers fast-forward only when it is actually possible", () => {
    expect(dropActions(branch, other, false).map((a) => a.id)).not.toContain("fastForward");
    expect(dropActions(branch, other, true).map((a) => a.id)).toContain("fastForward");
  });

  it("offers nothing when a branch lands on itself", () => {
    expect(dropActions(branch, branch, true)).toEqual([]);
  });

  it("offers squash when a commit lands on another commit", () => {
    const ids = dropActions(commit, { kind: "commit", id: "def" }, false).map((a) => a.id);
    expect(ids).toContain("squash");
  });

  it("offers nothing when a commit lands on itself", () => {
    expect(dropActions(commit, commit, false)).toEqual([]);
  });

  it("offers nothing for mixed kinds, which has no meaning", () => {
    expect(dropActions(commit, branch, false)).toEqual([]);
    expect(dropActions(branch, commit, false)).toEqual([]);
  });

  it("labels every action with the names involved", () => {
    for (const action of dropActions(branch, other, true)) {
      expect(action.title).toContain("feature");
      expect(action.title).toContain("main");
    }
  });
});

// Merge and rebase run on the checked-out branch. With `dev` checked out, "Merge feature
// into main" made a merge commit in dev, and "Rebase feature onto main" rebased dev.
describe("a drop on branches that are not checked out", () => {
  const feature = { kind: "branch", id: "feature" } as const;
  const main = { kind: "branch", id: "main" } as const;
  const find = (head: string | null, id: string) =>
    dropActions(feature, main, false, head).find((action) => action.id === id);

  it("offers the merge only into the checked-out branch", () => {
    expect(find("dev", "merge")?.disabled).toBeTruthy();
    expect(find("main", "merge")?.disabled).toBeUndefined();
  });

  it("offers the rebase only of the checked-out branch", () => {
    expect(find("dev", "rebase")?.disabled).toBeTruthy();
    expect(find("feature", "rebase")?.disabled).toBeUndefined();
  });
});
