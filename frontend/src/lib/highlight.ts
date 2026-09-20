import { classHighlighter, highlightTree } from "@lezer/highlight";
import { parser as cpp } from "@lezer/cpp";
import { parser as css } from "@lezer/css";
import { parser as html } from "@lezer/html";
import { parser as javascript } from "@lezer/javascript";
import { parser as json } from "@lezer/json";
import { parser as python } from "@lezer/python";
import { parser as rust } from "@lezer/rust";
import type { LRParser } from "@lezer/lr";

export interface Token {
  start: number;
  end: number;
  cls: string;
}

export const MAX_HIGHLIGHT_LINES = 5000;

const PARSERS: Record<string, LRParser> = {
  c: cpp,
  cpp,
  css,
  html,
  javascript,
  json,
  python,
  rust,
  typescript: javascript.configure({ dialect: "ts" }) as LRParser,
};

/** Parsed as one document so a comment spanning lines survives onto the next one. */
export function highlightLines(lines: readonly string[], language: string | null): Token[][] {
  const empty = lines.map(() => [] as Token[]);
  const parser = language ? PARSERS[language] : undefined;
  if (!parser || lines.length === 0 || lines.length > MAX_HIGHLIGHT_LINES) return empty;

  const starts: number[] = [];
  let at = 0;
  for (const line of lines) {
    starts.push(at);
    at += line.length + 1;
  }

  try {
    const tree = parser.parse(lines.join("\n"));
    highlightTree(tree, classHighlighter, (from, to, cls) => {
      let line = upperBound(starts, from) - 1;
      while (line < lines.length && from < to) {
        const lineStart = starts[line] ?? 0;
        const lineEnd = lineStart + (lines[line]?.length ?? 0);
        const start = Math.max(from, lineStart);
        const end = Math.min(to, lineEnd);
        if (end > start) empty[line]?.push({ start: start - lineStart, end: end - lineStart, cls });
        from = lineEnd + 1;
        line += 1;
      }
    });
  } catch {
    return lines.map(() => [] as Token[]);
  }
  return empty;
}

function upperBound(starts: readonly number[], offset: number): number {
  let low = 0;
  let high = starts.length;
  while (low < high) {
    const mid = (low + high) >> 1;
    if ((starts[mid] ?? 0) <= offset) low = mid + 1;
    else high = mid;
  }
  return low;
}

export interface Piece {
  text: string;
  cls: string;
  changed: boolean;
}

/** One segmentation for both overlays, cut at every boundary either introduces. */
export function mergePieces(
  text: string,
  tokens: readonly Token[],
  inline: readonly [number, number][],
): Piece[] {
  const cuts = new Set<number>([0, text.length]);
  for (const token of tokens) {
    cuts.add(Math.min(token.start, text.length));
    cuts.add(Math.min(token.end, text.length));
  }
  for (const [from, to] of inline) {
    cuts.add(Math.min(from, text.length));
    cuts.add(Math.min(to, text.length));
  }

  const bounds = [...cuts].sort((a, b) => a - b);
  const pieces: Piece[] = [];
  for (let i = 0; i < bounds.length - 1; i++) {
    const start = bounds[i] ?? 0;
    const end = bounds[i + 1] ?? 0;
    if (end <= start) continue;
    pieces.push({
      text: text.slice(start, end),
      cls: tokens.find((t) => t.start <= start && t.end >= end)?.cls ?? "",
      changed: inline.some(([from, to]) => from <= start && to >= end),
    });
  }
  return pieces;
}
