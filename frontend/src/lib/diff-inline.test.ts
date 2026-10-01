import { describe, expect, it } from "vitest";
import { blockInline, tokenize } from "./diff-inline";

const slice = (line: string, ranges: [number, number][]) => ranges.map(([a, b]) => line.slice(a, b));

describe("tokenize", () => {
  it("splits identifiers, numbers, operators and whitespace runs", () => {
    expect(tokenize("a_b  + 12").map((t) => t.text)).toEqual(["a_b", "  ", "+", " ", "12"]);
  });
  it("keeps Unicode letters together and offsets in UTF-16 units", () => {
    const t = tokenize("😀 привет");
    expect(t.map((x) => x.text)).toEqual(["😀", " ", "привет"]);
    expect(t[2]!.start).toBe(3);
  });
});

describe("blockInline", () => {
  it("compares the whole block, so a moved word is found across lines", () => {
    const r = blockInline(["foo(a,", "    b)"], ["foo(a,", "    b, c)"])!;
    expect(r.left).toEqual([[], []]);
    expect(slice("    b, c)", r.right[1]!)).toEqual([", c"]);
  });

  it("highlights only the new tokens of a line, not the unchanged comment lines", () => {
    const l = ["// keep", "call(x, y, w);"];
    const r = ["// keep", "call(x, std::back_inserter(z), y, w);"];
    const out = blockInline(l, r)!;
    expect(out.right[0]).toEqual([]);
    expect(slice(r[1]!, out.right[1]!).join("")).toContain("std::back_inserter(z),");
    expect(out.left[0]).toEqual([]);
  });

  it("merges ranges split by one short unchanged token", () => {
    const out = blockInline(["aaaa bbbb stay stay1 stay2"], ["cccc dddd stay stay1 stay2"])!;
    expect(out.left[0]).toEqual([[0, 9]]);
    expect(out.right[0]).toEqual([[0, 9]]);
  });

  it("keeps ranges apart when a longer token separates them", () => {
    const out = blockInline(["aaaa middle bbbb s1 s2 s3 s4"], ["cccc middle dddd s1 s2 s3 s4"])!;
    expect(out.left[0]).toEqual([[0, 4], [12, 16]]);
  });

  it("gives a mostly rewritten line only its background", () => {
    const out = blockInline(["one two three four"], ["one 2 3 4"])!;
    expect(out.left[0]).toEqual([]);
    expect(out.right[0]).toEqual([]);
  });

  it("trims whitespace from the ranges", () => {
    const out = blockInline(["a b c d e f x y"], ["a b c d e f  z y"])!;
    expect(slice("a b c d e f  z y", out.right[0]!)).toEqual(["z"]);
  });

  it("highlights an indentation-only change only when asked", () => {
    expect(blockInline(["  foo(x);"], ["    foo(x);"])!.right[0]).toEqual([]);
    expect(blockInline(["  foo(x);"], ["    foo(x);"], { indent: true })!.right[0]).toEqual([[0, 4]]);
  });

  it("returns ranges in UTF-16 units", () => {
    const out = blockInline(["😀 a b c d e"], ["😀 a b c d X e"])!;
    expect(slice("😀 a b c d X e", out.right[0]!)).toEqual(["X"]);
  });

  it("handles pure insert and pure delete sides", () => {
    expect(blockInline([], ["x"])!.right[0]).toEqual([]);
    expect(blockInline(["x"], [])!.left).toEqual([[]]);
  });

  it("bails out above the size cap and stays fast", () => {
    const lines = Array.from({ length: 5000 }, (_, i) => `let value${i} = compute(${i}, "text");`);
    const other = lines.map((l) => l.replace("compute", "calc"));
    const t0 = performance.now();
    expect(blockInline(lines, other)).toBeNull();
    expect(performance.now() - t0).toBeLessThan(50);
  });

  it("stays fast on a block under the cap with scattered edits", () => {
    const lines = Array.from({ length: 400 }, (_, i) => `let value${i} = compute(${i}, "text");`);
    const other = lines.map((l, i) => (i % 7 === 0 ? l.replace("compute", "calc") : l));
    const t0 = performance.now();
    const out = blockInline(lines, other)!;
    expect(performance.now() - t0).toBeLessThan(50);
    expect(slice(other[0]!, out.right[0]!)).toEqual(["calc"]);
    expect(out.right[1]).toEqual([]);
  });
});
