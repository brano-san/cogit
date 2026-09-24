import { describe, expect, it } from "vitest";
import { planFor } from "./disk-change";

describe("planFor", () => {
  it("asks for nothing when nothing changed", () => {
    expect(planFor([])).toEqual({
      refs: false,
      worktree: false,
      hooks: false,
      authors: false,
      cascade: false,
    });
  });

  it("keeps a hook edit out of the cascade", () => {
    expect(planFor(["hooks"])).toEqual({
      refs: false,
      worktree: false,
      hooks: true,
      authors: false,
      cascade: false,
    });
  });

  it("re-reads the repository when a ref moved", () => {
    expect(planFor(["refs"]).refs).toBe(true);
    expect(planFor(["head"]).refs).toBe(true);
  });

  it("leaves the repository alone when only the working tree moved", () => {
    const plan = planFor(["workingTree"]);
    expect(plan.refs).toBe(false);
    expect(plan.worktree).toBe(true);
    expect(plan.cascade).toBe(true);
  });

  it("treats an index write as a working-tree change", () => {
    expect(planFor(["index"]).worktree).toBe(true);
  });

  it("runs the cascade for a stash or a config write without touching the file lists", () => {
    const plan = planFor(["stash", "config"]);
    expect(plan.cascade).toBe(true);
    expect(plan.worktree).toBe(false);
    expect(plan.refs).toBe(false);
  });

  it("merges a burst into one plan instead of one plan each", () => {
    // What `git commit` actually produces: index, working tree, HEAD, refs.
    expect(planFor(["index", "workingTree", "head", "refs"])).toEqual({
      refs: true,
      worktree: true,
      hooks: false,
      authors: false,
      cascade: true,
    });
  });

  it("still runs the cascade when a hook edit arrives with real work", () => {
    const plan = planFor(["hooks", "refs"]);
    expect(plan.hooks).toBe(true);
    expect(plan.cascade).toBe(true);
  });

  it("does not care how often the same kind repeats", () => {
    expect(planFor(["index", "index", "index"])).toEqual(planFor(["index"]));
  });

  it("names the authors anew when .mailmap changed, without reopening the repository", () => {
    const plan = planFor(["workingTree", "mailmap"]);
    expect(plan.authors).toBe(true);
    expect(plan.refs).toBe(false);
    expect(plan.worktree).toBe(true);
  });

  it("leaves the authors alone for an ordinary working-tree edit", () => {
    expect(planFor(["workingTree"]).authors).toBe(false);
  });
});
