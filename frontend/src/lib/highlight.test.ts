import { describe, expect, it } from "vitest";
import { MAX_HIGHLIGHT_LINES, highlightLines, mergePieces } from "./highlight";

function classesOf(tokens: { cls: string }[][], line: number): string[] {
  return (tokens[line] ?? []).map((t) => t.cls);
}

describe("highlightLines", () => {
  it("returns nothing for a language it does not know", () => {
    expect(highlightLines(["fn main() {}"], "klingon")).toEqual([[]]);
  });

  it("returns nothing when no language is given", () => {
    expect(highlightLines(["fn main() {}"], null)).toEqual([[]]);
  });

  it("marks a Rust keyword", () => {
    const tokens = highlightLines(["fn main() {}"], "rust");
    expect(classesOf(tokens, 0).some((c) => c.includes("keyword"))).toBe(true);
  });

  it("gives one entry per input line", () => {
    expect(highlightLines(["let a = 1;", "let b = 2;"], "rust")).toHaveLength(2);
  });

  it("keeps offsets relative to their own line", () => {
    const tokens = highlightLines(["let a = 1;", "fn main() {}"], "rust");
    const keyword = tokens[1]?.find((t) => t.cls.includes("keyword"));
    expect(keyword?.start).toBe(0);
    expect(keyword?.end).toBe(2);
  });

  it("carries a block comment across the line it started on", () => {
    const tokens = highlightLines(["/* start", "still comment */", "let a = 1;"], "rust");

    expect(classesOf(tokens, 1).some((c) => c.includes("comment"))).toBe(true);
  });

  it("does not highlight beyond the end of a line", () => {
    const lines = ["let a = 1;", "let bb = 2;"];
    const tokens = highlightLines(lines, "rust");
    for (const [index, line] of lines.entries()) {
      for (const token of tokens[index] ?? []) {
        expect(token.end).toBeLessThanOrEqual(line.length);
        expect(token.start).toBeGreaterThanOrEqual(0);
      }
    }
  });

  it("gives up on a file too large to be worth parsing", () => {
    const many = Array.from({ length: MAX_HIGHLIGHT_LINES + 1 }, () => "let a = 1;");
    expect(highlightLines(many, "rust").every((line) => line.length === 0)).toBe(true);
  });

  it("handles an empty input", () => {
    expect(highlightLines([], "rust")).toEqual([]);
  });

  it("survives text that does not parse", () => {
    expect(() => highlightLines(["!!! not rust @@@"], "rust")).not.toThrow();
  });
});

describe("mergePieces", () => {
  it("returns the whole line when there is nothing to mark", () => {
    expect(mergePieces("plain", [], [])).toEqual([{ text: "plain", cls: "", changed: false, hit: false, start: 0 }]);
  });

  it("applies a syntax class to its own range only", () => {
    const pieces = mergePieces("fn x", [{ start: 0, end: 2, cls: "tok-keyword" }], []);
    expect(pieces.map((p) => p.cls)).toEqual(["tok-keyword", ""]);
  });

  it("marks a changed word without losing its syntax class", () => {
    const pieces = mergePieces("let a", [{ start: 0, end: 3, cls: "tok-keyword" }], [[0, 3]]);
    expect(pieces[0]).toEqual({ text: "let", cls: "tok-keyword", changed: true, hit: false, start: 0 });
  });

  it("cuts at every boundary either overlay introduces", () => {
    const pieces = mergePieces("abcdef", [{ start: 0, end: 4, cls: "t" }], [[2, 6]]);
    expect(pieces.map((p) => p.text)).toEqual(["ab", "cd", "ef"]);
    expect(pieces.map((p) => p.changed)).toEqual([false, true, true]);
  });

  it("puts the pieces back together into the original text", () => {
    const text = "const answer = 42;";
    const pieces = mergePieces(text, [{ start: 0, end: 5, cls: "tok-keyword" }], [[15, 17]]);
    expect(pieces.map((p) => p.text).join("")).toBe(text);
  });

  it("clamps a range that runs past the end", () => {
    expect(mergePieces("ab", [], [[1, 99]]).map((p) => p.text).join("")).toBe("ab");
  });

  it("handles an empty line", () => {
    expect(mergePieces("", [], [])).toEqual([]);
  });

  it("marks a search hit and nothing around it", () => {
    const pieces = mergePieces("find me here", [], [], [[5, 7]]);

    expect(pieces.map((p) => p.text)).toEqual(["find ", "me", " here"]);
    expect(pieces.map((p) => p.hit)).toEqual([false, true, false]);
  });

  it("keeps a hit that lands inside a changed word marked as both", () => {
    const pieces = mergePieces("alpha", [], [[0, 5]], [[0, 5]]);

    expect(pieces[0]).toEqual({ text: "alpha", cls: "", changed: true, hit: true, start: 0 });
  });

  it("cuts at the hit boundary as well as the other two", () => {
    const pieces = mergePieces("abcdef", [{ start: 0, end: 2, cls: "t" }], [[2, 4]], [[3, 6]]);

    expect(pieces.map((p) => p.text).join("")).toBe("abcdef");
    expect(pieces.map((p) => p.hit)).toEqual([false, false, true, true]);
  });
});
