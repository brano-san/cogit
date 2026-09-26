import { highlightedRows, type FoldEntry } from "./diff-fold";
import type { SidePair } from "./diff-rows";
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

function whole(text: string, language: string | null): Side {
  const lines = text.split("\n");
  if (text.endsWith("\n")) lines.pop();
  return { lines, tokens: highlightLines(lines, language), at: null };
}

/**
 * Tokens for both sides of a diff, parsed once per diff. A side the backend sent whole is
 * parsed whole: the hunks alone are pieces of a file, a piece that starts inside a function
 * is broken code to the parser, and its tokens come out cut in the wrong places (R-530).
 * A side too long to send is parsed from the loaded rows, and past the limit from the rows
 * on show (R-472).
 */
export function diffTokens(diff: DiffText, shown: () => readonly FoldEntry[]): DiffTokens {
  let rows: readonly DiffRow[] | null = null;
  const loaded = () => (rows ??= highlightedRows(diff.hunks, shown, MAX_HIGHLIGHT_LINES));
  return {
    old: diff.oldText !== null ? whole(diff.oldText, diff.language) : fragment(loaded(), diff.language, true),
    new: diff.newText !== null ? whole(diff.newText, diff.language) : fragment(loaded(), diff.language, false),
  };
}

/** `null` when the side has no such line, or quotes it otherwise than `text`. */
function sideLine(side: Side, line: number, text: string): Token[] | null {
  const index = side.at ? side.at.get(line) : line - 1;
  if (index === undefined || side.lines[index] !== text) return null;
  return side.tokens[index] ?? [];
}

export function rowTokens(tokens: DiffTokens, row: DiffRow): Token[] {
  if (row.kind === "delete") return sideLine(tokens.old, row.old, row.text) ?? [];
  if (row.kind === "insert") return sideLine(tokens.new, row.new, row.text) ?? [];
  if (row.kind === "context") {
    return sideLine(tokens.old, row.old, row.text) ?? sideLine(tokens.new, row.new, row.text) ?? [];
  }
  return [];
}

export function cellTokens(tokens: DiffTokens, pair: SidePair, side: "left" | "right"): Token[] {
  const cell = pair[side];
  if (!cell) return [];
  if (cell.kind === "delete") return sideLine(tokens.old, cell.line, cell.text) ?? [];
  if (cell.kind === "insert") return sideLine(tokens.new, cell.line, cell.text) ?? [];
  const twin = side === "left" ? pair.right : pair.left;
  const own = side === "left" ? tokens.old : tokens.new;
  const other = side === "left" ? tokens.new : tokens.old;
  return sideLine(own, cell.line, cell.text) ?? (twin ? sideLine(other, twin.line, cell.text) : null) ?? [];
}
