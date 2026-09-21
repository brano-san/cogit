import { describe, expect, it } from "vitest";
import { flatten, toggle, type TreeNode } from "./tree";

interface Row extends TreeNode {
  label: string;
}

const tree: Row[] = [
  { id: "a", depth: 0, label: "a", children: true },
  { id: "a/1", depth: 1, label: "a/1", children: true },
  { id: "a/1/x", depth: 2, label: "a/1/x" },
  { id: "a/2", depth: 1, label: "a/2" },
  { id: "b", depth: 0, label: "b", children: true },
  { id: "b/1", depth: 1, label: "b/1" },
];

describe("flatten", () => {
  it("shows everything when nothing is collapsed", () => {
    expect(flatten(tree, new Set()).map((row) => row.id)).toEqual([
      "a",
      "a/1",
      "a/1/x",
      "a/2",
      "b",
      "b/1",
    ]);
  });

  it("hides the children of a collapsed node but keeps the node", () => {
    expect(flatten(tree, new Set(["a/1"])).map((row) => row.id)).toEqual([
      "a",
      "a/1",
      "a/2",
      "b",
      "b/1",
    ]);
  });

  it("hides a whole branch when its root is collapsed", () => {
    expect(flatten(tree, new Set(["a"])).map((row) => row.id)).toEqual(["a", "b", "b/1"]);
  });

  it("does not let a collapsed node inside a hidden branch swallow its siblings", () => {
    // Collapsing both 'a' and 'a/1' must not walk past 'a' and eat 'b'.
    expect(flatten(tree, new Set(["a", "a/1"])).map((row) => row.id)).toEqual([
      "a",
      "b",
      "b/1",
    ]);
  });

  it("marks each row with whether it is open", () => {
    const rows = flatten(tree, new Set(["a"]));
    expect(rows[0]?.open).toBe(false);
    expect(rows.find((row) => row.id === "b")?.open).toBe(true);
  });

  it("gives a leaf no open state at all", () => {
    const rows = flatten(tree, new Set());
    expect(rows.find((row) => row.id === "a/2")?.open).toBeUndefined();
  });

  it("returns nothing for an empty tree", () => {
    expect(flatten([], new Set())).toEqual([]);
  });
});

describe("toggle", () => {
  it("collapses an open node", () => {
    expect(toggle(new Set(), "a").has("a")).toBe(true);
  });

  it("opens a collapsed one", () => {
    expect(toggle(new Set(["a"]), "a").has("a")).toBe(false);
  });

  it("leaves the original set alone", () => {
    const before = new Set(["a"]);
    toggle(before, "b");
    expect([...before]).toEqual(["a"]);
  });
});
