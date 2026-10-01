import { beforeAll, describe, expect, it } from "vitest";
import { diffTokens, paneTokens, unifiedTokens } from "./diff-highlight";
import { MAX_HIGHLIGHT_LINES, highlightLines, loadLanguage } from "./highlight";
import type { DiffRow, Hunk } from "./ipc";

beforeAll(() => loadLanguage("rust"));

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
describe("diffTokens", () => {
  it("colours a hunk that starts inside a function as the whole file colours it", () => {
    const tokens = diffTokens(
      { hunks: [HUNK], language: "rust", oldText: text(OLD), newText: text(NEW) },
      () => [],
    );

    expect(unifiedTokens(tokens, { type: "context", old: 3, new: 3, text: OLD[2]! })).toEqual(highlightLines(OLD, "rust")[2]);
    expect(unifiedTokens(tokens, { type: "insert", old: null, new: 4, text: NEW[3]! })).toEqual(highlightLines(NEW, "rust")[3]);
    expect(unifiedTokens(tokens, { type: "context", old: 3, new: 3, text: OLD[2]! }).find((token) => token.start === 8)?.cls).toBe("tok-propertyName");
  });

  it("parses the loaded rows of a side whose text did not come", () => {
    const tokens = diffTokens({ hunks: [HUNK], language: "rust", oldText: null, newText: null }, () => []);

    expect(unifiedTokens(tokens, { type: "delete", old: 4, new: null, text: OLD[3]! }).length).toBeGreaterThan(0);
  });

  it("parses a side of exactly the limit, final newline and all", () => {
    const lines = Array.from({ length: MAX_HIGHLIGHT_LINES }, () => "let a = 1;");
    const row = context(MAX_HIGHLIGHT_LINES, MAX_HIGHLIGHT_LINES, lines[0]!);
    const tokens = diffTokens(
      { hunks: [{ ...HUNK, rows: [row] }], language: "rust", oldText: text(lines), newText: text(lines) },
      () => [],
    );

    expect(unifiedTokens(tokens, { type: "context", old: MAX_HIGHLIGHT_LINES, new: MAX_HIGHLIGHT_LINES, text: row.kind === "context" ? row.text : "" })).toEqual(highlightLines(["let a = 1;"], "rust")[0]);
  });

  it("colours a right-hand context line by the old line when the new one reads otherwise", () => {
    // Ignoring whitespace, a context row quotes the old line on both sides.
    const spaced = [...NEW.slice(0, 2), "        recorded:  p.recorded,", ...NEW.slice(3)];
    const tokens = diffTokens(
      { hunks: [HUNK], language: "rust", oldText: text(OLD), newText: text(spaced) },
      () => [],
    );

    expect(paneTokens(tokens, "right", 3, OLD[2]!, 3)).toEqual(highlightLines(OLD, "rust")[2]);
  });

  it("colours each side of a changed pair from its own side", () => {
    const tokens = diffTokens(
      { hunks: [HUNK], language: "rust", oldText: text(OLD), newText: text(NEW) },
      () => [],
    );

    expect(paneTokens(tokens, "left", 4, OLD[3]!)).toEqual(highlightLines(OLD, "rust")[3]);
    expect(paneTokens(tokens, "right", 4, NEW[3]!)).toEqual(highlightLines(NEW, "rust")[3]);
    expect(paneTokens(tokens, "left", 4, NEW[3]!)).toEqual([]);
  });
});
