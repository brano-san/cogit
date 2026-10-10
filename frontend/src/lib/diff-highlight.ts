import { highlightedRows, type FoldEntry } from "./diff-fold";
import { MAX_HIGHLIGHT_LINES, highlightLines, type Token } from "./highlight";
import type { DiffRow, Hunk } from "./ipc";

export interface DiffText {
  hunks: readonly Hunk[];
  language: string | null;
  oldText: string | null;
  newText: string | null;
}

interface Side {
  lines: readonly string[];
  tokens: readonly Token[][];
  /** Line number to index; `null` when `lines` is the whole side, line `n` at `n - 1`. */
  at: ReadonlyMap<number, number> | null;
}

export interface DiffTokens {
  old: Side;
  new: Side;
}

function fragment(rows: readonly DiffRow[], language: string | null, old: boolean): Side {
  const lines: string[] = [];
  const at = new Map<number, number>();
  for (const row of rows) {
    if (row.kind === "context") at.set(old ? row.old : row.new, lines.push(row.text) - 1);
    else if (row.kind === "delete" && old) at.set(row.old, lines.push(row.text) - 1);
    else if (row.kind === "insert" && !old) at.set(row.new, lines.push(row.text) - 1);
  }
  return { lines, tokens: highlightLines(lines, language), at };
}

/** The last text parsed whole, per side. Typing in the diff's editor re-diffs the file at
    every pause, and the old side stays as it was: its parse, the bulk of a re-diff, is
    taken from here. ponytail: one text per side; two views of different files take turns. */
const parsed: Record<"old" | "new", { text: string; language: string | null; side: Side } | null> = { old: null, new: null };

function whole(text: string, language: string | null, which: "old" | "new"): Side {
  const last = parsed[which];
  if (last && last.text === text && last.language === language) return last.side;
  const lines = text.split("\n");
  if (text.endsWith("\n")) lines.pop();
  const side = { lines, tokens: highlightLines(lines, language), at: null };
  parsed[which] = { text, language, side };
  return side;
}

/**
 * Tokens for both sides of a diff, each parsed when first read: the editor draws the new
 * side itself. A side the backend sent whole is parsed whole: the hunks alone are pieces of
 * a file, a piece that starts inside a function is broken code to the parser, and its tokens
 * come out cut in the wrong places (R-530). A side too long to send is parsed from the
 * loaded rows, and past the limit from the rows on show (R-472).
 */
export function diffTokens(diff: DiffText, shown: () => readonly FoldEntry[]): DiffTokens {
  let rows: readonly DiffRow[] | null = null;
  const loaded = () => (rows ??= highlightedRows(diff.hunks, shown, MAX_HIGHLIGHT_LINES));
  let old: Side | undefined;
  let next: Side | undefined;
  return {
    get old() {
      return (old ??= diff.oldText !== null ? whole(diff.oldText, diff.language, "old") : fragment(loaded(), diff.language, true));
    },
    get new() {
      return (next ??= diff.newText !== null ? whole(diff.newText, diff.language, "new") : fragment(loaded(), diff.language, false));
    },
  };
}

/** `null` when the side has no such line, or quotes it otherwise than `text`. */
function sideLine(side: Side, line: number, text: string): Token[] | null {
  const index = side.at ? side.at.get(line) : line - 1;
  if (index === undefined || side.lines[index] !== text) return null;
  return side.tokens[index] ?? [];
}

/** A line of a pane. `twin` is the same line's number on the other side, for an unchanged
    line whose own side reads otherwise (whitespace ignored: both quote the old line). The
    other side is read only then: reading it parses it. */
export function paneTokens(
  tokens: DiffTokens,
  side: "left" | "right",
  line: number,
  text: string,
  twin: number | null = null,
): Token[] {
  const own = sideLine(side === "left" ? tokens.old : tokens.new, line, text);
  if (own !== null || twin === null) return own ?? [];
  return sideLine(side === "left" ? tokens.new : tokens.old, twin, text) ?? [];
}

export interface UnifiedLineRef {
  type: "context" | "delete" | "insert";
  old: number | null;
  new: number | null;
  text: string;
}

export function unifiedTokens(tokens: DiffTokens, row: UnifiedLineRef): Token[] {
  const old = row.type === "insert" || row.old === null ? null : sideLine(tokens.old, row.old, row.text);
  if (old !== null) return old;
  return (row.new === null ? null : sideLine(tokens.new, row.new, row.text)) ?? [];
}
