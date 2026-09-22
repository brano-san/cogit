import { describe, expect, it } from "vitest";
import type { HealthFinding } from "$lib/ipc";
import { groupFindings, placeOf, visibleWarnings } from "./health";

const ignoreCase = (module: string): HealthFinding => ({
  module,
  issue: { kind: "ignoreCaseMismatch", configured: false, actual: true },
});

const worktree = (target: string, foreign: boolean): HealthFinding => ({
  module: "",
  issue: { kind: "danglingWorktree", name: "linked", target, foreign },
});

describe("placeOf", () => {
  it("names the repository alone, and a submodule in brackets after it", () => {
    expect(placeOf("dtv_device", "")).toBe("dtv_device");
    expect(placeOf("dtv_device", "import/libjam")).toBe("dtv_device [import/libjam]");
  });
});

describe("groupFindings", () => {
  it("has nothing to say about a healthy repository", () => {
    expect(groupFindings([], "dtv_device")).toEqual([]);
  });

  // The second machine: twenty-one repositories, one problem, not twenty-one warnings.
  it("turns the same problem in many submodules into one warning listing them", () => {
    const warnings = groupFindings(
      [ignoreCase(""), ignoreCase("import/libjam"), ignoreCase("import/kors")],
      "dtv_device",
    );
    expect(warnings).toHaveLength(1);
    expect(warnings[0]!.places.map((place) => place.label)).toEqual([
      "dtv_device",
      "dtv_device [import/libjam]",
      "dtv_device [import/kors]",
    ]);
  });

  it("keeps the two directions of a case mismatch apart, because the fix is opposite", () => {
    const warnings = groupFindings(
      [
        ignoreCase(""),
        { module: "x", issue: { kind: "ignoreCaseMismatch", configured: true, actual: false } },
      ],
      "repo",
    );
    expect(warnings).toHaveLength(2);
    expect(warnings.map((warning) => warning.fixes[0])).toEqual([
      "git config core.ignoreCase true",
      "git config core.ignoreCase false",
    ]);
  });

  it("offers repair for a worktree path from another system and prune for a folder that is gone", () => {
    const foreign = groupFindings([worktree("/home/user/work/wt/.git", true)], "repo")[0]!;
    const gone = groupFindings([worktree("E:/Work/wt/.git", false)], "repo")[0]!;
    expect(foreign.fixes.join(" ")).toContain("git worktree repair");
    expect(gone.fixes.join(" ")).toContain("git worktree prune");
  });

  it("shows where each broken path points, next to the place it belongs to", () => {
    const [warning] = groupFindings([worktree("E:/Work/wt/.git", false)], "repo");
    expect(warning!.places[0]!.detail).toBe("E:/Work/wt/.git");
  });

  it("puts broken paths before a setting that is merely wrong", () => {
    const warnings = groupFindings(
      [ignoreCase(""), worktree("E:/Work/wt/.git", false)],
      "repo",
    );
    expect(warnings[0]!.id).toMatch(/^danglingWorktree/);
  });

  it("gives a warning the same id however many places it covers, so Ignore sticks", () => {
    const one = groupFindings([ignoreCase("")], "repo")[0]!;
    const many = groupFindings([ignoreCase(""), ignoreCase("a"), ignoreCase("b")], "repo")[0]!;
    expect(one.id).toBe(many.id);
  });

  it("links to the documentation of the problem", () => {
    const [warning] = groupFindings([ignoreCase("")], "repo");
    expect(warning!.docs).toMatch(/^https:\/\/git-scm\.com\//);
  });
});

describe("visibleWarnings", () => {
  const warnings = groupFindings(
    [ignoreCase(""), worktree("E:/Work/wt/.git", false)],
    "repo",
  );

  it("leaves out what was ignored for this repository", () => {
    const ignored = new Set([warnings[0]!.id]);
    expect(visibleWarnings(warnings, ignored, new Set()).map((w) => w.id)).toEqual([
      warnings[1]!.id,
    ]);
  });

  it("leaves out what was put off until the repository is opened again", () => {
    const later = new Set([warnings[1]!.id]);
    expect(visibleWarnings(warnings, new Set(), later).map((w) => w.id)).toEqual([
      warnings[0]!.id,
    ]);
  });
});
