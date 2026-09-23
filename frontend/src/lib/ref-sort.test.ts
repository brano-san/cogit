import { describe, expect, it } from "vitest";
import {
  compareDated,
  compareNames,
  DEFAULT_REF_SORT,
  nextDateOrder,
  parseRefSort,
  type RefSort,
} from "./ref-sort";

const sorted = (names: string[], order: "natural" | "plain") =>
  [...names].sort((a, b) => compareNames(a, b, order));

describe("compareNames", () => {
  it("compares the numbers inside names as numbers", () => {
    expect(sorted(["v1.0.10", "v1.0.2", "v1.0.1"], "natural")).toEqual([
      "v1.0.1",
      "v1.0.2",
      "v1.0.10",
    ]);
  });

  it("keeps the old code-unit order when natural sorting is off", () => {
    expect(sorted(["v1.0.2", "v1.0.10", "B", "a"], "plain")).toEqual([
      "B",
      "a",
      "v1.0.10",
      "v1.0.2",
    ]);
  });

  it("gives names that differ only in case one fixed order", () => {
    expect(sorted(["main", "Main"], "natural")).toEqual(["Main", "main"]);
    expect(sorted(["Main", "main"], "natural")).toEqual(["Main", "main"]);
  });
});

describe("compareDated", () => {
  const newest: RefSort = { names: "natural", dates: "newest" };
  const oldest: RefSort = { names: "natural", dates: "oldest" };
  const refs = [
    { name: "b", date: 100 },
    { name: "a", date: 300 },
    { name: "c", date: undefined },
    { name: "d", date: 200 },
  ];
  const order = (sort: RefSort) =>
    [...refs].sort((x, y) => compareDated(x, y, sort)).map((ref) => ref.name);

  it("puts the newest tip first", () => {
    expect(order(newest)).toEqual(["a", "d", "b", "c"]);
  });

  it("puts the oldest tip first", () => {
    expect(order(oldest)).toEqual(["b", "d", "a", "c"]);
  });

  it("orders by name alone while dates are off", () => {
    expect(order(DEFAULT_REF_SORT)).toEqual(["a", "b", "c", "d"]);
  });

  it("breaks a tie in dates by name", () => {
    const tied = [
      { name: "v10", date: 5 },
      { name: "v9", date: 5 },
    ];
    expect([...tied].sort((x, y) => compareDated(x, y, newest)).map((r) => r.name)).toEqual([
      "v9",
      "v10",
    ]);
  });
});

describe("nextDateOrder", () => {
  it("cycles off, newest, oldest and back", () => {
    expect(nextDateOrder("off")).toBe("newest");
    expect(nextDateOrder("newest")).toBe("oldest");
    expect(nextDateOrder("oldest")).toBe("off");
  });
});

describe("parseRefSort", () => {
  it("falls back to natural names and no dates on anything unknown", () => {
    expect(parseRefSort(null)).toEqual(DEFAULT_REF_SORT);
    expect(parseRefSort({ names: "weird", dates: 3 })).toEqual(DEFAULT_REF_SORT);
  });

  it("keeps a stored choice", () => {
    expect(parseRefSort({ names: "plain", dates: "oldest" })).toEqual({
      names: "plain",
      dates: "oldest",
    });
  });
});
