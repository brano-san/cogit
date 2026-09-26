import { describe, expect, it } from "vitest";
import { compareLabel, compareUrl, parseCompare } from "./compare-params";

describe("compareUrl and parseCompare", () => {
  it("round-trips a commit comparison", () => {
    const url = compareUrl(3, "src/a.rs", { kind: "commitVsParent", oid: "abc" });
    expect(parseCompare(new URL(url, "http://x/").search)).toEqual({
      repo: 3,
      path: "src/a.rs",
      spec: { kind: "commitVsParent", oid: "abc" },
    });
  });

  it("round-trips a working-tree comparison", () => {
    const url = compareUrl(1, "a.txt", { kind: "workTreeVsIndex" });
    expect(parseCompare(new URL(url, "http://x/").search)?.spec).toEqual({
      kind: "workTreeVsIndex",
    });
  });

  it("survives a path with spaces and non-ASCII characters", () => {
    const path = "src/файл с пробелом.rs";
    const url = compareUrl(1, path, { kind: "workTreeVsIndex" });
    expect(parseCompare(new URL(url, "http://x/").search)?.path).toBe(path);
  });

  it("survives a path with a question mark and an ampersand", () => {
    const path = "weird/a?b&c.txt";
    const url = compareUrl(1, path, { kind: "workTreeVsIndex" });
    expect(parseCompare(new URL(url, "http://x/").search)?.path).toBe(path);
  });

  it("round-trips a two-commit comparison", () => {
    const url = compareUrl(1, "a.txt", { kind: "commitVsCommit", a: "aaa", b: "bbb" });
    expect(parseCompare(new URL(url, "http://x/").search)?.spec).toEqual({
      kind: "commitVsCommit",
      a: "aaa",
      b: "bbb",
    });
  });

  it("returns null for a query that is missing pieces", () => {
    expect(parseCompare("?repo=1")).toBeNull();
    expect(parseCompare("")).toBeNull();
  });

  it("returns null for a repo id that is not a number", () => {
    expect(parseCompare("?repo=x&path=a.txt&kind=workTreeVsIndex")).toBeNull();
  });

  it("returns null for a comparison kind it does not know", () => {
    expect(parseCompare("?repo=1&path=a.txt&kind=teleport")).toBeNull();
  });

  it("returns null when a commit comparison has no oid", () => {
    expect(parseCompare("?repo=1&path=a.txt&kind=commitVsParent")).toBeNull();
  });

  it("round-trips a past version against the working tree", () => {
    const url = compareUrl(2, "src/a.rs", { kind: "commitVsWorkTree", oid: "abc" });
    expect(parseCompare(new URL(url, "http://x/").search)?.spec).toEqual({
      kind: "commitVsWorkTree",
      oid: "abc",
    });
    expect(parseCompare("?repo=1&path=a.txt&kind=commitVsWorkTree")).toBeNull();
  });
});

// The window said "src/a.rs — workTreeVsIndex", and a past version against the disk never
// said which commit it was.
describe("compareLabel", () => {
  const oid = "0123456789abcdef0123456789abcdef01234567";
  const other = "fedcba9876543210fedcba9876543210fedcba98";

  it("names both sides in words", () => {
    expect(compareLabel({ kind: "workTreeVsIndex" })).toBe("Working tree ↔ index");
    expect(compareLabel({ kind: "indexVsHead" })).toBe("Staged ↔ HEAD");
  });

  it("names the commit it compares by its short id", () => {
    expect(compareLabel({ kind: "commitVsParent", oid })).toBe("0123456 ↔ parent");
    expect(compareLabel({ kind: "commitVsWorkTree", oid })).toBe("0123456 ↔ working tree");
    expect(compareLabel({ kind: "commitVsCommit", a: oid, b: other })).toBe("0123456 ↔ fedcba9");
  });
});
