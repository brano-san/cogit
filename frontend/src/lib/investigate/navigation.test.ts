import { describe, expect, it } from "vitest";
import type { FileRevision } from "$lib/ipc/investigate";
import { flatten, itemFor, makeSection, neighbour } from "./navigation";

function rev(oid: string, path: string, parents: string[] = []): FileRevision {
  return {
    oid,
    parents,
    summary: oid,
    author: "A",
    email: "a@x",
    timestamp: 0,
    path,
    previousPath: null,
    change: "modified",
  };
}

const sections = [
  makeSection("tale.txt", null, [rev("c3", "tale.txt", ["c2"]), rev("c2", "story.txt", ["c1"])], true),
  makeSection("donor.txt", "d9", [rev("d1", "donor.txt")], false),
];

describe("Navigation log", () => {
  it("heads each file's log with a header node, the working tree first when asked", () => {
    expect(flatten(sections).map((item) => item.kind)).toEqual([
      "header",
      "workingTree",
      "commit",
      "commit",
      "header",
      "commit",
    ]);
  });

  it("finds the row for a file at a version, or the working tree", () => {
    const items = flatten(sections);
    expect(itemFor(items, "tale.txt", null)).toBe(1);
    expect(itemFor(items, "story.txt", "c2")).toBe(3);
    expect(itemFor(items, "donor.txt", "d1")).toBe(5);
    expect(itemFor(items, "donor.txt", "nope")).toBe(-1);
  });

  it("falls back to the commit when the name differs, as after a rename", () => {
    expect(itemFor(flatten(sections), "tale.txt", "c2")).toBe(3);
  });

  it("steps to a newer or older row without leaving the section", () => {
    const items = flatten(sections);
    expect(neighbour(items, 2, 1)).toBe(3);
    expect(neighbour(items, 3, 1)).toBeNull();
    expect(neighbour(items, 2, -1)).toBe(1);
    expect(neighbour(items, 5, -1)).toBeNull();
  });
});
