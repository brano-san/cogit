import { describe, expect, it } from "vitest";
import { layoutGraph, segmentsOf } from "./graph";

describe("Navigation graph geometry", () => {
  it("draws the node mid-row, lines through, from above, from joins and to parents", () => {
    const drawn = segmentsOf(
      { lane: 0, through: [2], fromAbove: true, joins: [1], toParents: [0], width: 3 },
      10,
      20,
    );
    expect(drawn.node).toEqual([5, 10]);
    expect(drawn.lines).toEqual([
      [[25, 0], [25, 20]],
      [[5, 0], [5, 10]],
      [[15, 0], [5, 10]],
      [[5, 10], [5, 20]],
    ]);
  });
});

describe("Navigation graph lanes", () => {
  it("draws a linear history as one line", () => {
    const rows = layoutGraph([
      { oid: "c3", parents: ["c2"] },
      { oid: "c2", parents: ["c1"] },
      { oid: "c1", parents: [] },
    ]);
    expect(rows.map((row) => row.lane)).toEqual([0, 0, 0]);
    expect(rows.map((row) => row.fromAbove)).toEqual([false, true, true]);
    expect(rows.map((row) => row.toParents)).toEqual([[0], [0], []]);
  });

  it("joins rows whose real parents are not in the file's log", () => {
    const rows = layoutGraph([
      { oid: "c3", parents: ["x"] },
      { oid: "c2", parents: ["y"] },
      { oid: "c1", parents: [] },
    ]);
    expect(rows.map((row) => row.toParents)).toEqual([[0], [0], []]);
    expect(rows[2]!.fromAbove).toBe(true);
  });

  it("opens a second lane for a merge and closes it where the sides meet", () => {
    const rows = layoutGraph([
      { oid: "m", parents: ["a", "b"] },
      { oid: "a", parents: ["c"] },
      { oid: "b", parents: ["c"] },
      { oid: "c", parents: [] },
    ]);
    expect(rows[0]).toMatchObject({ lane: 0, toParents: [0, 1], width: 2 });
    expect(rows[1]).toMatchObject({ lane: 0, through: [1] });
    expect(rows[2]).toMatchObject({ lane: 1, through: [0], toParents: [1] });
    expect(rows[3]).toMatchObject({ lane: 0, joins: [1], toParents: [] });
  });

  it("gives an unrelated tip a lane of its own", () => {
    const rows = layoutGraph([
      { oid: "a2", parents: ["a1"] },
      { oid: "b1", parents: [] },
      { oid: "a1", parents: [] },
    ]);
    expect(rows[1]!.lane).toBe(1);
    expect(rows[1]!.through).toEqual([0]);
  });
});
