import { describe, expect, it } from "vitest";
import { MAX_CELLS, SIMILARITY_THRESHOLD, alignLines, similarity } from "./diff-align";

const rows = (pairs: ReturnType<typeof alignLines>) => pairs.map(([l, r]) => `${l ?? "-"}:${r ?? "-"}`).join(" ");

describe("similarity", () => {
  it("is 1 for equal lines and for two blank ones, 0 for a blank against text", () => {
    expect(similarity("a = b;", "a  =  b;")).toBe(1);
    expect(similarity("", "   ")).toBe(1);
    expect(similarity("", "x")).toBe(0);
  });

  it("does not count lines that share only punctuation, unless they are the same tokens", () => {
    expect(similarity("foo(a);", "bar(b);")).toBe(0);
    expect(similarity("}", "}")).toBe(1);
    expect(similarity("});", "});")).toBe(1);
    expect(similarity("foo(a);", "foo(b);")).toBeGreaterThanOrEqual(SIMILARITY_THRESHOLD);
  });

  it("puts the report's pair above the threshold", () => {
    expect(similarity("    if (not contains)", "    if (heardAt.empty()) {")).toBeGreaterThanOrEqual(SIMILARITY_THRESHOLD);
    expect(similarity("// heardAt mirrors result", "if (not contains)")).toBeLessThan(SIMILARITY_THRESHOLD);
  });
});

describe("alignLines", () => {
  const left = [
    "// walk the pending items",
    "if (not contains)",
    "std::transform(items.begin(), items.end(), std::back_inserter(result),",
    "toResult);",
    "stat.count += result.size();",
    "flush();",
  ];
  const right = [
    "// walk the pending items",
    "// heardAt mirrors result",
    "if (heardAt.empty()) {",
    "std::transform(items.begin(), items.end(), std::back_inserter(result), std::back_inserter(heardAt),",
    "toResult);",
    "}",
    "stat.count += result.size();",
    "stat.heard += heardAt.size();",
    "flush();",
  ];

  it("sets similar lines opposite each other and fills the rest", () => {
    expect(rows(alignLines(left, right))).toBe("0:0 -:1 1:2 2:3 3:4 -:5 4:6 -:7 5:8");
  });

  it("covers every line of both sides exactly once, in order", () => {
    const pairs = alignLines(left, right);
    expect(pairs.flatMap(([l]) => (l === null ? [] : [l]))).toEqual([0, 1, 2, 3, 4, 5]);
    expect(pairs.flatMap(([, r]) => (r === null ? [] : [r]))).toEqual([0, 1, 2, 3, 4, 5, 6, 7, 8]);
  });

  it("pairs by position and pads the end when nothing is alike", () => {
    expect(rows(alignLines(["aaa", "bbb"], ["xxx", "yyy", "zzz", "www"]))).toBe("0:0 1:1 -:2 -:3");
    expect(rows(alignLines(["aaa", "bbb", "ccc"], ["xxx"]))).toBe("0:0 1:- 2:-");
  });

  it("puts filler on the side that has no lines", () => {
    expect(rows(alignLines([], ["x", "y"]))).toBe("-:0 -:1");
    expect(rows(alignLines(["x"], []))).toBe("0:-");
  });

  it("falls back to positions above the cell cap, in time", () => {
    const side = (k: number) => Array.from({ length: k }, (_, i) => `value_${i} = compute(${i}) + other_${i % 7};`);
    const side2 = (k: number) => Array.from({ length: k }, (_, i) => `result_${i} += compute(${i}) * other_${i % 5};`);
    const big = alignLines(side(200), side2(200));
    expect(big).toHaveLength(200);
    expect(big.every(([l, r], k) => l === k && r === k)).toBe(true);
    expect(200 * 200).toBeGreaterThan(MAX_CELLS);

    // The largest block that is still matched: 100 × 100 long lines. Well under a frame.
    const long = (tag: string, k: number) =>
      Array.from({ length: k }, (_, i) => `${tag}(${Array.from({ length: 60 }, (_, t) => `arg${(i + t) % 13}`).join(", ")});`);
    const start = performance.now();
    alignLines(long("f", 100), long("g", 100));
    expect(performance.now() - start).toBeLessThan(50);
  });
});
