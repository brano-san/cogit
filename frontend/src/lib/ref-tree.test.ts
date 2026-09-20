import { describe, expect, it } from "vitest";
import type { Branch } from "$lib/ipc";
import { buildTree, matchesFilter } from "./ref-tree";

function branch(name: string): Branch {
  return {
    name,
    fullName: `refs/heads/${name}`,
    kind: "local",
    oid: "a".repeat(40),
    isHead: false,
    upstream: null,
    ahead: 0,
    behind: 0,
  };
}

describe("matchesFilter", () => {
  it("accepts everything when the filter is empty", () => {
    expect(matchesFilter("feature/auth", "")).toBe(true);
  });

  it("matches a substring anywhere in the name", () => {
    expect(matchesFilter("feature/auth/login", "auth")).toBe(true);
    expect(matchesFilter("feature/auth/login", "logout")).toBe(false);
  });

  it("ignores case", () => {
    expect(matchesFilter("Feature/Auth", "auth")).toBe(true);
  });

  it("matches across the slashes so a path can be typed whole", () => {
    expect(matchesFilter("feature/auth/login", "auth/log")).toBe(true);
  });
});

describe("buildTree", () => {
  it("keeps a flat name at the root", () => {
    const tree = buildTree([branch("main")]);
    expect(tree).toEqual([{ label: "main", depth: 0, branch: tree[0]?.branch }]);
  });

  it("nests a name with slashes under its folders", () => {
    const rows = buildTree([branch("feature/auth/login")]);

    expect(rows.map((r) => [r.label, r.depth])).toEqual([
      ["feature", 0],
      ["auth", 1],
      ["login", 2],
    ]);
  });

  it("shares a folder between siblings instead of repeating it", () => {
    const rows = buildTree([branch("feature/a"), branch("feature/b")]);

    expect(rows.filter((r) => r.label === "feature")).toHaveLength(1);
    expect(rows).toHaveLength(3);
  });

  it("marks folders as folders and leaves as branches", () => {
    const rows = buildTree([branch("feature/a")]);

    expect(rows[0]?.branch).toBeUndefined();
    expect(rows[1]?.branch?.name).toBe("feature/a");
  });

  it("keeps the order the branches arrived in", () => {
    const rows = buildTree([branch("b"), branch("a")]);
    expect(rows.map((r) => r.label)).toEqual(["b", "a"]);
  });

  it("handles an empty list", () => {
    expect(buildTree([])).toEqual([]);
  });

  it("does not create an empty folder from a trailing slash", () => {
    expect(buildTree([branch("odd/")]).map((r) => r.label)).toEqual(["odd"]);
  });
});
