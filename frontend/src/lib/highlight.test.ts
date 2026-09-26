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

  // DF-053: without the JSX dialect `</div>` opened a regular expression and the colours
  // after the tag were wrong.
  it("reads JSX tags in .tsx and .jsx as tags, not as a regular expression", () => {
    const line = 'return <div className="a">x</div>;';
    for (const language of ["tsx", "jsx"]) {
      const tokens = highlightLines([line], language)[0] ?? [];
      const at = (text: string) => tokens.find((t) => t.start === line.indexOf(text))?.cls;
      expect(at("div"), language).toBe("tok-typeName");
      expect(tokens.some((t) => t.cls.includes("string2")), language).toBe(false);
    }
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

describe("mergePieces on a minified line", () => {
  // One line of minified JavaScript: tens of thousands of alternating tokens. Every piece
  // searched the whole token list, and the window froze for seconds (rule of 50 ms).
  const line = "a=1;".repeat(75_000);
  const tokens = Array.from({ length: 150_000 }, (_, i) => ({
    start: i * 2,
    end: i * 2 + 1,
    cls: i % 2 ? "tok-number" : "tok-variableName",
  }));

  it("cuts a 300 KB line in one pass", () => {
    const started = performance.now();
    mergePieces(line, tokens, [[10, 20]], [[100, 104]]);
    expect(performance.now() - started).toBeLessThan(1000);
  });

  it("keeps the changed words and the hits of a line too long to colour", () => {
    const pieces = mergePieces(line, tokens, [[10, 20]], [[100, 104]]);

    expect(pieces.every((piece) => piece.cls === "")).toBe(true);
    expect(pieces.filter((piece) => piece.changed).map((piece) => piece.text).join("")).toBe(line.slice(10, 20));
    expect(pieces.filter((piece) => piece.hit).map((piece) => piece.text).join("")).toBe(line.slice(100, 104));
    expect(pieces.map((piece) => piece.text).join("")).toBe(line);
  });

  it("colours a short line exactly as before", () => {
    const text = "let x = 1;";
    const short = [
      { start: 0, end: 3, cls: "tok-keyword" },
      { start: 4, end: 5, cls: "tok-variableName" },
      { start: 8, end: 9, cls: "tok-number" },
    ];
    const pieces = mergePieces(text, short, [[4, 9]], [[8, 10]]);

    expect(pieces).toEqual([
      { text: "let", cls: "tok-keyword", changed: false, hit: false, start: 0 },
      { text: " ", cls: "", changed: false, hit: false, start: 3 },
      { text: "x", cls: "tok-variableName", changed: true, hit: false, start: 4 },
      { text: " = ", cls: "", changed: true, hit: false, start: 5 },
      { text: "1", cls: "tok-number", changed: true, hit: true, start: 8 },
      { text: ";", cls: "", changed: false, hit: true, start: 9 },
    ]);
  });
});
