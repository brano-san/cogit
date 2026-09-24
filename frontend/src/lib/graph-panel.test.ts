import { describe, expect, it } from "vitest";
import { textX } from "$lib/graph-geometry";
import { GRAPH_MIN_SUBJECT_CHARS, graphPanelMinWidth, subjectRoom } from "./graph-panel";

describe("the Graph panel's minimum width", () => {
  it("is one lane of graph plus about 35 characters of subject", () => {
    expect(GRAPH_MIN_SUBJECT_CHARS).toBe(35);
    expect(graphPanelMinWidth()).toBe(`calc(${textX(1)}px + 35ch)`);
  });

  it("reserves the same characters for the subject in every row before the graph is cut", () => {
    expect(subjectRoom(7)).toBe(35 * 7);
  });
});
