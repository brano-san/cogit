/**
 * Pairing the lines of a `changed` block for the Aligned layout (08 §12.2): which left line
 * stands opposite which right line. An order-preserving matching that maximizes the summed
 * similarity of the matched pairs (the Needleman-Wunsch table with free gaps), where a pair
 * counts only at `SIMILARITY_THRESHOLD` or above. Lines left unmatched stand opposite filler.
 * With no pair at all the block is paired by position and the shorter side is padded at the
 * end, so a rewrite of unrelated lines still reads as "these replace those".
 */

/** Dice ratio of the two lines' tokens at or above which two lines count as the same line,
    edited. 0.4 admits `if (not contains)` ↔ `if (heardAt.empty()) {` (0.43: `if`, `(`, `)` shared
    of 5 and 9 tokens); lines that share only punctuation fall below it. */
export const SIMILARITY_THRESHOLD = 0.4;
/** The table is `n·m` cells, each a bag comparison. Above this a block is paired by position
    (a 100 × 100 block is the ceiling; the worst case measured ≈ 15 ms). */
export const MAX_CELLS = 10_000;
/** Longer lines are compared by their first tokens only. */
export const MAX_TOKENS = 200;

/** `[left index, right index]`; `null` is filler. In reading order, every line exactly once. */
export type Pair = [number | null, number | null];

const WORD = /^[\p{L}\p{N}_]/u;
const TOKEN = /[\p{L}\p{N}_]+|[^\s\p{L}\p{N}_]/gu;

function bag(text: string): { counts: Map<string, number>; size: number } {
  const counts = new Map<string, number>();
  let size = 0;
  for (const m of text.matchAll(TOKEN)) {
    counts.set(m[0], (counts.get(m[0]) ?? 0) + 1);
    if (++size === MAX_TOKENS) break;
  }
  return { counts, size };
}

/** 2·|A ∩ B| / (|A| + |B|) over the multisets of tokens; two blank lines are the same line.
    Lines that share nothing but punctuation (`foo(a);` and `bar(b);`) are not alike: at least
    one identifier or number must be common, unless the tokens are the same. */
export function similarity(a: string, b: string): number {
  return ratio(bag(a), bag(b));
}

function ratio(a: ReturnType<typeof bag>, b: ReturnType<typeof bag>): number {
  if (a.size === 0 && b.size === 0) return 1;
  if (a.size === 0 || b.size === 0) return 0;
  const [small, large] = a.counts.size <= b.counts.size ? [a.counts, b.counts] : [b.counts, a.counts];
  let common = 0;
  let words = 0;
  for (const [token, n] of small) {
    const shared = Math.min(n, large.get(token) ?? 0);
    common += shared;
    if (shared > 0 && WORD.test(token)) words += shared;
  }
  if (words === 0 && common !== a.size) return 0;
  return (2 * common) / (a.size + b.size);
}

function positional(n: number, m: number): Pair[] {
  return Array.from({ length: Math.max(n, m) }, (_, k) => [k < n ? k : null, k < m ? k : null]);
}

export function alignLines(left: readonly string[], right: readonly string[]): Pair[] {
  const n = left.length;
  const m = right.length;
  if (n === 0 || m === 0 || n * m > MAX_CELLS) return positional(n, m);

  const a = left.map(bag);
  const b = right.map(bag);
  const w = m + 1;
  const sim = new Float64Array(n * m);
  const best = new Float64Array((n + 1) * w);
  for (let i = 1; i <= n; i++) {
    for (let j = 1; j <= m; j++) {
      const s = ratio(a[i - 1]!, b[j - 1]!);
      sim[(i - 1) * m + (j - 1)] = s;
      let v = Math.max(best[(i - 1) * w + j]!, best[i * w + j - 1]!);
      if (s >= SIMILARITY_THRESHOLD) v = Math.max(v, best[(i - 1) * w + j - 1]! + s);
      best[i * w + j] = v;
    }
  }

  const matches: [number, number][] = [];
  for (let i = n, j = m; i > 0 && j > 0; ) {
    const s = sim[(i - 1) * m + (j - 1)]!;
    if (s >= SIMILARITY_THRESHOLD && best[i * w + j] === best[(i - 1) * w + j - 1]! + s) {
      matches.push([i - 1, j - 1]);
      i--;
      j--;
    } else if (best[i * w + j] === best[(i - 1) * w + j]!) i--;
    else j--;
  }
  if (matches.length === 0) return positional(n, m);

  matches.reverse();
  const out: Pair[] = [];
  let i = 0;
  let j = 0;
  for (const [mi, mj] of [...matches, [n, m] as [number, number]]) {
    for (; i < mi; i++) out.push([i, null]);
    for (; j < mj; j++) out.push([null, j]);
    if (mi < n) out.push([mi, mj]);
    i = mi + 1;
    j = mj + 1;
  }
  return out;
}
