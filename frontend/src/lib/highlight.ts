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
  jsx: javascript.configure({ dialect: "jsx" }) as LRParser,
  tsx: javascript.configure({ dialect: "ts jsx" }) as LRParser,
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
  /** Part of a search match, so the view can mark it without a fourth pass. */
  hit: boolean;
  /** Offset of this piece in the line, which is how the view tells one hit from another. */
  start: number;
}

/** Past this a line is minified code: its tens of thousands of token spans cost the
    renderer more than the colour tells anyone. Changed words and search hits still show. */
export const MAX_HIGHLIGHT_CHARS = 10_000;

/**
 * One segmentation for all three overlays, cut at every boundary any of them introduces.
 *
 * One sweep over the sorted cuts: looking up the covering range for every piece made a
 * minified line quadratic.
 */
export function mergePieces(
  text: string,
  tokens: readonly Token[],
  inline: readonly [number, number][],
  hits: readonly [number, number][] = [],
): Piece[] {
  const length = text.length;
  const spans = length > MAX_HIGHLIGHT_CHARS ? [] : byStart(tokens);
  const cuts = new Set<number>([0, length]);
  for (const token of spans) {
    cuts.add(Math.min(token.start, length));
    cuts.add(Math.min(token.end, length));
  }
  for (const [from, to] of [...inline, ...hits]) {
    cuts.add(Math.min(from, length));
    cuts.add(Math.min(to, length));
  }

  const bounds = [...cuts].sort((a, b) => a - b);
  const changed = coverage(bounds, inline, length);
  const hit = coverage(bounds, hits, length);
  const pieces: Piece[] = [];
  let next = 0;
  for (let i = 0; i < bounds.length - 1; i++) {
    const start = bounds[i] ?? 0;
    const end = bounds[i + 1] ?? 0;
    if (end <= start) continue;
    while (next < spans.length && Math.min(spans[next]!.end, length) <= start) next += 1;
    const span = spans[next];
    pieces.push({
      text: text.slice(start, end),
      cls: span && span.start <= start && span.end >= end ? span.cls : "",
      changed: changed[i] ?? false,
      hit: hit[i] ?? false,
      start,
    });
  }
  return pieces;
}

function byStart(tokens: readonly Token[]): readonly Token[] {
  for (let i = 1; i < tokens.length; i++) {
    if (tokens[i]!.start < tokens[i - 1]!.start) return [...tokens].sort((a, b) => a.start - b.start);
  }
  return tokens;
}

/** Per piece between `bounds`, whether any of `ranges` covers it. Every range edge is a
    cut, so a range that touches a piece covers all of it. */
function coverage(bounds: readonly number[], ranges: readonly [number, number][], length: number): boolean[] {
  const covered: boolean[] = [];
  if (ranges.length === 0) return covered;
  const at = new Map(bounds.map((bound, index) => [bound, index]));
  const delta = new Array<number>(bounds.length).fill(0);
  for (const [from, to] of ranges) {
    const start = at.get(Math.min(from, length));
    const end = at.get(Math.min(to, length));
    if (start === undefined || end === undefined || start >= end) continue;
    delta[start]! += 1;
    delta[end]! -= 1;
  }
  let open = 0;
  for (let i = 0; i < bounds.length - 1; i++) {
    open += delta[i]!;
    covered.push(open > 0);
  }
  return covered;
}
