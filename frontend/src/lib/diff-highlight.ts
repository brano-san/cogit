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

/** A line of a pane. `twin` is the same line's number on the other side, for an unchanged
    line whose own side reads otherwise (whitespace ignored: both quote the old line). */
export function paneTokens(
  tokens: DiffTokens,
  side: "left" | "right",
  line: number,
  text: string,
  twin: number | null = null,
): Token[] {
  const own = side === "left" ? tokens.old : tokens.new;
  const other = side === "left" ? tokens.new : tokens.old;
  return sideLine(own, line, text) ?? (twin === null ? null : sideLine(other, twin, text)) ?? [];
}

export interface UnifiedLineRef {
  type: "context" | "delete" | "insert";
  old: number | null;
  new: number | null;
  text: string;
}

export function unifiedTokens(tokens: DiffTokens, row: UnifiedLineRef): Token[] {
  const old = row.old === null ? null : sideLine(tokens.old, row.old, row.text);
  const next = row.new === null ? null : sideLine(tokens.new, row.new, row.text);
  return (row.type === "insert" ? next : (old ?? next)) ?? [];
}
