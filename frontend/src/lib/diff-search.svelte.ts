import { searchRows, stepHit, type SearchHit, type SearchRow } from "$lib/diff-rows";

/** The scan walks every row of the file. At typing speed that is a frame lost per letter
    on a large diff, so the search runs on what was typed a moment ago. */
export const SETTLE_MS = 120;

/**
 * The Find bar of the diff panel: what was typed, what has been searched for, and which
 * hit the counter points at. Lives beside the component rather than inside it so the
 * stepping and the span bookkeeping can be tested without rendering a diff.
 */
export class DiffSearch {
  showing = $state(false);
  /** What the user sees in the box, updated on every keystroke. */
  query = $state("");

  #applied = $state("");
  #at = $state(0);
  #timer: ReturnType<typeof setTimeout> | undefined;
  /** Replaced by the constructor; the derived fields below are declared after it and
      TypeScript wants it to hold something by then. */
  #rows: () => readonly SearchRow[] = () => [];

  readonly hits: SearchHit[] = $derived(searchRows(this.#rows(), this.#applied));
  readonly current: SearchHit | null = $derived(this.hits[this.#at] ?? null);

  readonly #spans: Map<string, [number, number][]> = $derived.by(() => {
    const byCell = new Map<string, [number, number][]>();
    for (const hit of this.hits) {
      const key = `${hit.index}:${hit.side}`;
      const spans = byCell.get(key) ?? [];
      spans.push([hit.from, hit.to]);
      byCell.set(key, spans);
    }
    return byCell;
  });

  constructor(rows: () => readonly SearchRow[]) {
    this.#rows = rows;
  }

  /** What the hits were computed from; empty while the pause has not elapsed. */
  get applied(): string {
    return this.#applied;
  }

  get at(): number {
    return this.#at;
  }

  setQuery(text: string): void {
    this.query = text;
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => (this.#applied = text), SETTLE_MS);
  }

  open(): void {
    this.showing = true;
  }

  close(): void {
    clearTimeout(this.#timer);
    this.showing = false;
    this.query = "";
    this.#applied = "";
    this.#at = 0;
  }

  /** A new file, or a new search: the counter goes back to the first hit. */
  rewind(): void {
    this.#at = 0;
  }

  go(delta: number, reveal: (index: number) => void): void {
    if (this.hits.length === 0) return;
    this.#at = stepHit(this.hits, this.#at, delta);
    const hit = this.hits[this.#at];
    if (hit) reveal(hit.index);
  }

  spansFor(index: number, side: "left" | "right"): [number, number][] {
    return this.#spans.get(`${index}:${side}`) ?? [];
  }

  /** The one hit the counter is pointing at, told apart from the rest it looks like. */
  isCurrent(index: number, side: "left" | "right", start: number): boolean {
    const hit = this.current;
    if (!hit || hit.index !== index || hit.side !== side) return false;
    return start >= hit.from && start < hit.to;
  }
}
