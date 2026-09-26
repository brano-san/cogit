import { describe, expect, it } from "vitest";
import { textX } from "$lib/graph-geometry";
import { GRAPH_MIN_SUBJECT_CHARS, emptyHistory, graphPanelMinWidth, showsWorkingTree, subjectRoom } from "./graph-panel";

describe("the Graph panel's minimum width", () => {
  it("is one lane of graph plus about 35 characters of subject", () => {
    expect(GRAPH_MIN_SUBJECT_CHARS).toBe(35);
    expect(graphPanelMinWidth()).toBe(`calc(${textX(1)}px + 35ch)`);
  });

  it("reserves the same characters for the subject in every row before the graph is cut", () => {
    expect(subjectRoom(7)).toBe(35 * 7);
  });
});

// A filter without matches said "No commits yet", as if the repository had none (doc/05 §7).
describe("an empty history", () => {
  it("names the filter and offers to clear it when the filter emptied it", () => {
    expect(emptyHistory(true, null)).toMatchObject({ title: "No commits match the filter", clears: true });
    expect(emptyHistory(true, [])).toMatchObject({ clears: true });
  });

  it("says no branch is ticked when none is", () => {
    expect(emptyHistory(false, [])).toMatchObject({ title: "No branches shown", clears: false });
  });

  it("is a repository without commits otherwise", () => {
    expect(emptyHistory(false, null)).toMatchObject({ title: "No commits yet", clears: false });
    expect(emptyHistory(false, ["HEAD"])).toMatchObject({ title: "No commits yet" });
  });
});

describe("showsWorkingTree", () => {
  const clean = { staged: 0, unstaged: 0, untracked: 0, conflicted: 0 };

  it("always shows the row while the setting is on", () => {
    expect(showsWorkingTree(true, clean, { kind: "clean" })).toBe(true);
  });

  it("leaves the row out of a clean working tree when the setting is off", () => {
    expect(showsWorkingTree(false, clean, { kind: "clean" })).toBe(false);
    expect(showsWorkingTree(false, clean, { kind: "detachedHead", oid: "abc" })).toBe(false);
  });

  it("brings it back for any change", () => {
    for (const change of ["staged", "unstaged", "untracked", "conflicted"] as const) {
      expect(showsWorkingTree(false, { ...clean, [change]: 1 }, { kind: "clean" }), change).toBe(true);
    }
  });

  it("keeps it while an operation is stopped half way", () => {
    expect(showsWorkingTree(false, clean, { kind: "merging" })).toBe(true);
  });

  it("keeps it until the status is read", () => {
    expect(showsWorkingTree(false, undefined, undefined)).toBe(true);
  });
});
