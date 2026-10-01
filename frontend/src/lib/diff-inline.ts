/** Word-level highlight of a `changed` block: the whole left text against the whole right
    text, so a token that moved between lines is not marked, and a comment kept as it was is
    not painted because its neighbor line changed. Pure; the ranges are an overlay layer,
    syntax colors are independent of them. */

export type Range = [number, number];

export interface InlineOptions {
  /** Highlight whitespace-only changes (indentation, runs inside a line). */
  indent?: boolean;
}

export interface InlineResult {
  /** Per line of the left side, UTF-16 ranges of removed words. */
  left: Range[][];
  /** Per line of the right side, UTF-16 ranges of added words. */
  right: Range[][];
}

/** Above this many tokens (both sides) the block keeps only its line backgrounds. */
export const INLINE_MAX_TOKENS = 60_000;
/** Edit distance cap of the token diff; a block this different is a rewrite. */
const MAX_EDITS = 1500;
/** More changed words (identifiers, numbers; punctuation does not count) than this share
    of a line: background only. */
const LINE_THRESHOLD = 0.5;
/** An unchanged token this short between two changes is absorbed into them. */
const SHORT_TOKEN = 3;

export interface Token {
  text: string;
  /** Offset in its line, UTF-16 units; `-1` for a line break. */
  start: number;
  line: number;
}

const TOKEN = /\s+|[\p{L}\p{N}_]+|[^\s\p{L}\p{N}_]/gu;
const BLANK = /^\s+$/;
const WORD = /[\p{L}\p{N}]/u;

export function tokenize(line: string, index = 0): Token[] {
  return Array.from(line.matchAll(TOKEN), (m) => ({ text: m[0], start: m.index, line: index }));
}

function tokens(lines: readonly string[]): Token[] {
  const out: Token[] = [];
  lines.forEach((line, i) => {
    if (i > 0) out.push({ text: "\n", start: -1, line: i });
    for (const t of tokenize(line, i)) out.push(t);
  });
  return out;
}

/** Myers' O(ND) diff over token ids. Returns which tokens are changed on each side and
    which token of `b` each unchanged token of `a` pairs with; `null` past `MAX_EDITS`. */
function myers(a: Int32Array, b: Int32Array): { da: Uint8Array; db: Uint8Array; pair: Int32Array } | null {
  const n = a.length;
  const m = b.length;
  // No prefix/suffix trimming: it can pair the wrong occurrences (an appended
  // `std::back_inserter(x),` would mark the old argument as removed).
  const da = new Uint8Array(n);
  const db = new Uint8Array(m);
  const pair = new Int32Array(n).fill(-1);

  const N = n;
  const M = m;
  const max = Math.min(N + M, MAX_EDITS);
  const off = max + 1;
  const v = new Int32Array(2 * max + 3);
  const trace: Int32Array[] = [];
  let found = -1;
  for (let d = 0; d <= max && found < 0; d++) {
    trace.push(v.slice());
    for (let k = -d; k <= d; k += 2) {
      let x = k === -d || (k !== d && v[off + k - 1]! < v[off + k + 1]!) ? v[off + k + 1]! : v[off + k - 1]! + 1;
      let y = x - k;
      while (x < N && y < M && a[x] === b[y]) {
        x++;
        y++;
      }
      v[off + k] = x;
      if (x >= N && y >= M) {
        found = d;
        break;
      }
    }
  }
  if (found < 0) return null;

  // Walk back: at each `d` the snapshot in trace[d] holds the frontier of d - 1.
  let x = N;
  let y = M;
  for (let d = found; d > 0; d--) {
    const prev = trace[d]!;
    const k = x - y;
    const down = k === -d || (k !== d && prev[off + k - 1]! < prev[off + k + 1]!);
    const pk = down ? k + 1 : k - 1;
    const px = prev[off + pk]!;
    const py = px - pk;
    while (x > (down ? px : px + 1) && y > (down ? py + 1 : py)) {
      x--;
      y--;
      pair[x] = y;
    }
    if (down) db[py] = 1;
    else da[px] = 1;
    x = px;
    y = py;
  }
  while (x > 0 && y > 0) {
    x--;
    y--;
    pair[x] = y;
  }
  for (let i = 0; i < n; i++) if (pair[i]! < 0) da[i] = 1;
  return { da, db, pair };
}

function lineRanges(
  toks: readonly Token[],
  changed: Uint8Array,
  lines: number,
  indent: boolean,
): Range[][] {
  const out: Range[][] = Array.from({ length: lines }, () => []);
  let i = 0;
  while (i < toks.length) {
    const line = toks[i]!.line;
    let end = i;
    while (end < toks.length && toks[end]!.line === line && (end === i || toks[end]!.start >= 0)) end++;
    // `toks[i]` may be the line break that opens the line: it belongs to none.
    const from = toks[i]!.start < 0 ? i + 1 : i;
    if (from < end) out[line] = oneLine(toks, changed, from, end, indent);
    i = end;
  }
  return out;
}

function oneLine(toks: readonly Token[], changed: Uint8Array, from: number, end: number, indent: boolean): Range[] {
  let words = 0;
  let hit = 0;
  for (let i = from; i < end; i++) {
    if (!WORD.test(toks[i]!.text)) continue;
    words++;
    if (changed[i]) hit++;
  }
  if (words > 0 && hit / words > LINE_THRESHOLD) return [];

  const ranges: Range[] = [];
  for (let i = from; i < end; ) {
    if (!changed[i]) {
      i++;
      continue;
    }
    let stop = i;
    while (stop < end && changed[stop]) stop++;
    let a = i;
    let z = stop;
    while (a < z && BLANK.test(toks[a]!.text)) a++;
    while (z > a && BLANK.test(toks[z - 1]!.text)) z--;
    if (a === z) {
      if (indent) ranges.push([toks[i]!.start, toks[stop - 1]!.start + toks[stop - 1]!.text.length]);
    } else {
      ranges.push([toks[a]!.start, toks[z - 1]!.start + toks[z - 1]!.text.length]);
    }
    i = stop;
  }
  return ranges;
}

/** `null` when the block is too big or too different: only line backgrounds then. */
export function blockInline(
  left: readonly string[],
  right: readonly string[],
  opts: InlineOptions = {},
): InlineResult | null {
  const lt = tokens(left);
  const rt = tokens(right);
  if (lt.length + rt.length > INLINE_MAX_TOKENS) return null;
  const ids = new Map<string, number>();
  const id = (t: Token) => ids.get(t.text) ?? (ids.set(t.text, ids.size), ids.size - 1);
  const a = Int32Array.from(lt, id);
  const b = Int32Array.from(rt, id);
  const diff = myers(a, b);
  if (!diff) return null;
  const { da, db, pair } = diff;

  // cleanupSemantic: a short token alone between two changes on both sides joins them.
  for (let i = 1; i < a.length - 1; i++) {
    const j = pair[i]!;
    if (j < 1 || j >= b.length - 1 || da[i]) continue;
    const t = lt[i]!.text;
    if (t.length > SHORT_TOKEN || t === "\n") continue;
    if (da[i - 1] && da[i + 1] && db[j - 1] && db[j + 1]) {
      da[i] = 1;
      db[j] = 1;
    }
  }
  const indent = opts.indent ?? false;
  return {
    left: lineRanges(lt, da, left.length, indent),
    right: lineRanges(rt, db, right.length, indent),
  };
}
