import { describe, expect, it } from "vitest";
import { textX } from "$lib/graph-geometry";
import { GRAPH_MIN_SUBJECT_CHARS, emptyHistory, graphPanelMinWidth, subjectRoom } from "./graph-panel";

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
