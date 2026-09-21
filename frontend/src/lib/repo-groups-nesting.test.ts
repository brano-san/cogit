import { describe, expect, it } from "vitest";
import { addGroup, groupRows, mergeGroups, nest, removeGroup, type RepoGroups } from "./repo-groups";

function twoGroups(): { groups: RepoGroups; outer: string; inner: string } {
  const first = addGroup({ order: [], names: {}, of: {}, under: {} }, "Work");
  const second = addGroup(first.groups, "Client");
  return {
    groups: second.groups,
    outer: first.id as string,
    inner: second.id as string,
  };
}

describe("nest", () => {
  it("puts one group inside another", () => {
    const { groups, outer, inner } = twoGroups();
    expect(nest(groups, inner, outer).under[inner]).toBe(outer);
  });

  it("takes a group back to the top level", () => {
    const { groups, outer, inner } = twoGroups();
    const nested = nest(groups, inner, outer);
    expect(nest(nested, inner, null).under[inner]).toBeUndefined();
  });

  it("refuses to put a group inside itself", () => {
    const { groups, outer } = twoGroups();
    expect(nest(groups, outer, outer)).toBe(groups);
  });

  it("refuses a cycle, however long the way round", () => {
    const { groups, outer, inner } = twoGroups();
    const nested = nest(groups, inner, outer);
    // `outer` inside `inner` would leave both unreachable from the top.
    expect(nest(nested, outer, inner)).toBe(nested);
  });

  it("refuses a parent that is not a group", () => {
    const { groups, inner } = twoGroups();
    expect(nest(groups, inner, "nope")).toBe(groups);
  });
});

describe("groupRows with nesting", () => {
  it("draws a child under its parent, one level deeper", () => {
    const { groups, outer, inner } = twoGroups();
    const rows = groupRows(nest(groups, inner, outer), [], new Set());

    const headings = rows.filter((row) => row.kind === "group");
    expect(headings.map((row) => (row.kind === "group" ? row.id : ""))).toEqual([outer, inner]);
    expect(headings.map((row) => (row.kind === "group" ? row.depth : -1))).toEqual([0, 1]);
  });

  it("hides a nested group when its parent is collapsed", () => {
    const { groups, outer, inner } = twoGroups();
    const rows = groupRows(nest(groups, inner, outer), [], new Set([outer]));

    expect(rows.some((row) => row.kind === "group" && row.id === inner)).toBe(false);
  });

  it("counts a parent's own repositories, not its children's", () => {
    const { groups, outer, inner } = twoGroups();
    let next = nest(groups, inner, outer);
    next = { ...next, of: { "C:/a": outer, "C:/b": inner } };

    const rows = groupRows(next, ["C:/a", "C:/b"], new Set());
    const parent = rows.find((row) => row.kind === "group" && row.id === outer);
    expect(parent?.kind === "group" ? parent.count : -1).toBe(1);
  });

  it("puts a repository of a nested group under that group", () => {
    const { groups, outer, inner } = twoGroups();
    let next = nest(groups, inner, outer);
    next = { ...next, of: { "C:/b": inner } };

    const rows = groupRows(next, ["C:/b"], new Set());
    const at = rows.findIndex((row) => row.kind === "repo");
    const above = rows[at - 1];
    expect(above?.kind === "group" ? above.id : null).toBe(inner);
  });
});

describe("removeGroup with nesting", () => {
  it("lifts the children of a group that goes to where it was", () => {
    const { groups, outer, inner } = twoGroups();
    const nested = nest(groups, inner, outer);
    expect(removeGroup(nested, outer).under[inner]).toBeUndefined();
  });
});

describe("mergeGroups with nesting", () => {
  it("reads the nesting back", () => {
    const { groups, outer, inner } = twoGroups();
    const nested = nest(groups, inner, outer);
    expect(mergeGroups(JSON.parse(JSON.stringify(nested))).under[inner]).toBe(outer);
  });

  it("drops a parent that is not in the list any more", () => {
    const stored = { order: ["a"], names: { a: "A" }, of: {}, under: { a: "gone" } };
    expect(mergeGroups(stored).under.a).toBeUndefined();
  });

  it("drops a stored cycle rather than hiding both groups", () => {
    const stored = {
      order: ["a", "b"],
      names: { a: "A", b: "B" },
      of: {},
      under: { a: "b", b: "a" },
    };
    const merged = mergeGroups(stored);
    expect(merged.under.a === "b" && merged.under.b === "a").toBe(false);
  });
});
