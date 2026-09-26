import { describe, expect, it } from "vitest";
import { cellTokens, diffTokens, rowTokens } from "./diff-highlight";
import { MAX_HIGHLIGHT_LINES, highlightLines } from "./highlight";
import type { DiffRow, Hunk } from "./ipc";
import type { SideCell } from "./diff-rows";

const OLD = [
  "fn pointer(p: &P) -> D {",
  "    let shown = D {",
  "        recorded: p.recorded,",
  "        previous: p.previous,",
  "    };",
  "    shown",
  "}",
];
const NEW = [...OLD.slice(0, 3), "        previous: p.before,", ...OLD.slice(4)];

const context = (old: number, next: number, text: string): DiffRow => ({ kind: "context", old, new: next, text, noNewline: false });
const change = (kind: "delete" | "insert", line: number, text: string): DiffRow =>
  kind === "delete"
    ? { kind, old: line, text, inline: [], moved: false, moveId: null, moveScope: null, noNewline: false }
    : { kind, new: line, text, inline: [], moved: false, moveId: null, moveScope: null, noNewline: false };

/** One line of context: the hunk starts inside the struct literal, as a real one does. */
const HUNK: Hunk = {
  oldStart: 3,
  oldLines: 3,
  newStart: 3,
  newLines: 3,
  header: "@@ -3,3 +3,3 @@",
  rows: [
    context(3, 3, OLD[2]!),
    change("delete", 4, OLD[3]!),
    change("insert", 4, NEW[3]!),
    context(5, 5, OLD[4]!),
  ],
};

const text = (lines: readonly string[]) => lines.join("\n") + "\n";
const cell = (kind: SideCell["kind"], line: number, value: string): SideCell => ({
  kind,
  line,
  text: value,
  inline: [],
  moved: false,
  moveId: null,
  noNewline: false,
});

describe("diffTokens", () => {
  it("colours a hunk that starts inside a function as the whole file colours it", () => {
    const tokens = diffTokens(
      { hunks: [HUNK], language: "rust", oldText: text(OLD), newText: text(NEW) },
      () => [],
    );

    expect(rowTokens(tokens, HUNK.rows[0]!)).toEqual(highlightLines(OLD, "rust")[2]);
    expect(rowTokens(tokens, HUNK.rows[2]!)).toEqual(highlightLines(NEW, "rust")[3]);
    expect(rowTokens(tokens, HUNK.rows[0]!).find((token) => token.start === 8)?.cls).toBe("tok-propertyName");
  });

  it("parses the loaded rows of a side whose text did not come", () => {
    const tokens = diffTokens({ hunks: [HUNK], language: "rust", oldText: null, newText: null }, () => []);

    expect(rowTokens(tokens, HUNK.rows[1]!).length).toBeGreaterThan(0);
  });

  it("parses a side of exactly the limit, final newline and all", () => {
    const lines = Array.from({ length: MAX_HIGHLIGHT_LINES }, () => "let a = 1;");
    const row = context(MAX_HIGHLIGHT_LINES, MAX_HIGHLIGHT_LINES, lines[0]!);
    const tokens = diffTokens(
      { hunks: [{ ...HUNK, rows: [row] }], language: "rust", oldText: text(lines), newText: text(lines) },
      () => [],
    );

    expect(rowTokens(tokens, row)).toEqual(highlightLines(["let a = 1;"], "rust")[0]);
  });

  it("colours a right-hand context line by the old line when the new one reads otherwise", () => {
    // Ignoring whitespace, a context row quotes the old line on both sides.
    const spaced = [...NEW.slice(0, 2), "        recorded:  p.recorded,", ...NEW.slice(3)];
    const tokens = diffTokens(
      { hunks: [HUNK], language: "rust", oldText: text(OLD), newText: text(spaced) },
      () => [],
    );
    const pair = { left: cell("context", 3, OLD[2]!), right: cell("context", 3, OLD[2]!) };

    expect(cellTokens(tokens, pair, "right")).toEqual(highlightLines(OLD, "rust")[2]);
  });

  it("colours each side of a changed pair from its own side", () => {
    const tokens = diffTokens(
      { hunks: [HUNK], language: "rust", oldText: text(OLD), newText: text(NEW) },
      () => [],
    );
    const pair = { left: cell("delete", 4, OLD[3]!), right: cell("insert", 4, NEW[3]!) };

    expect(cellTokens(tokens, pair, "left")).toEqual(highlightLines(OLD, "rust")[3]);
    expect(cellTokens(tokens, pair, "right")).toEqual(highlightLines(NEW, "rust")[3]);
    expect(cellTokens(tokens, { left: null, right: pair.right }, "left")).toEqual([]);
  });
});
