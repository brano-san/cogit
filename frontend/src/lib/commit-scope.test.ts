import { describe, expect, it } from "vitest";
import type { FileEntry } from "$lib/ipc";
import { commitScope } from "./commit-scope";

function entry(path: string): FileEntry {
  return { path, oldPath: null, status: "modified", mode: "plain", modeChange: null, similarity: null };
}

const staged = [entry("a.rs"), entry("b.rs"), entry("c.ts")];

describe("commitScope", () => {
  it("commits everything when no filter is active", () => {
    const scope = commitScope(staged, "");

    expect(scope.label).toBe("Commit 3");
    expect(scope.hidden).toBe(0);
    expect(scope.paths).toBeNull();
  });

  it("says how many are shown when a filter is active", () => {
    expect(commitScope(staged, "*.rs").label).toBe("Commit 2 shown");
  });

  it("counts what the filter hides", () => {
    expect(commitScope(staged, "*.rs").hidden).toBe(1);
  });

  it("lists exactly the paths that will be committed", () => {
    expect(commitScope(staged, "*.rs").paths).toEqual(["a.rs", "b.rs"]);
  });

  it("passes no path list when nothing is filtered out", () => {
    expect(commitScope(staged, "*").paths).toBeNull();
  });

  it("warns only while something is hidden", () => {
    expect(commitScope(staged, "*.rs").warning).toContain("1 more");
    expect(commitScope(staged, "").warning).toBeNull();
  });

  it("uses the singular for one hidden file", () => {
    expect(commitScope(staged, "*.rs").warning).toContain("1 more changed file is hidden");
  });

  it("uses the plural for several", () => {
    expect(commitScope(staged, "a.rs").warning).toContain("2 more changed files are hidden");
  });

  it("is empty when the filter matches nothing", () => {
    const scope = commitScope(staged, "nothing");
    expect(scope.paths).toEqual([]);
    expect(scope.label).toBe("Commit 0 shown");
  });

  it("handles an empty staging area", () => {
    expect(commitScope([], "").label).toBe("Commit 0");
  });
});

// The list filters by name, path and status, by regex, by content and by the view's
// switches; the scope guessed with a glob of its own. A filter for "modified" showed both
// staged files and committed "0 shown", and a commit of no paths is a commit of all.
describe("commitScope with the list's own rows", () => {
  it("counts what the list shows, not what a glob would", () => {
    const scope = commitScope(staged, "modified", ["a.rs", "b.rs", "c.ts"]);
    expect(scope.hidden).toBe(0);
    expect(scope.paths).toBeNull();
  });

  it("commits only the rows on screen", () => {
    expect(commitScope(staged, "a", ["a.rs"]).paths).toEqual(["a.rs"]);
  });

  it("commits nothing when the filter hides every staged file", () => {
    const scope = commitScope(staged, "zzz", []);
    expect(scope.empty).toBe(true);
    expect(scope.paths).toEqual([]);
  });
});
