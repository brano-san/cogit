import { describe, expect, it } from "vitest";
import { GraphFolds } from "./graph-folds.svelte";

type RepoId = import("$lib/ipc").RepoId;

describe("opened merges", () => {
  it("stay open while the repository does and close when another is shown", () => {
    const folds = new GraphFolds();
    folds.forRepo(1 as RepoId);
    folds.toggle("m");
    folds.forRepo(1 as RepoId);
    expect([...folds.expanded]).toEqual(["m"]);

    folds.forRepo(2 as RepoId);
    expect(folds.expanded.size).toBe(0);
    folds.forRepo(1 as RepoId);
    expect(folds.expanded.size).toBe(0);
  });

  it("toggle one merge at a time", () => {
    const folds = new GraphFolds();
    folds.toggle("a");
    folds.toggle("b");
    folds.toggle("a");
    expect([...folds.expanded]).toEqual(["b"]);
  });
});
