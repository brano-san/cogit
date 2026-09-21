import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DiffSearch, SETTLE_MS } from "./diff-search.svelte";
import type { SearchRow } from "./diff-rows";

const ROWS: SearchRow[] = [
  ["alpha beta", null],
  ["gamma", "beta delta"],
  ["beta", null],
];

function search(rows: readonly SearchRow[] = ROWS) {
  return new DiffSearch(() => rows);
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("DiffSearch", () => {
  it("finds nothing until the typing pause has elapsed", () => {
    const find = search();
    find.setQuery("beta");

    expect(find.hits).toHaveLength(0);
    vi.advanceTimersByTime(SETTLE_MS);
    expect(find.hits).toHaveLength(3);
  });

  it("only searches for the last thing typed", () => {
    const find = search();
    find.setQuery("b");
    vi.advanceTimersByTime(SETTLE_MS - 1);
    find.setQuery("gamma");
    vi.advanceTimersByTime(SETTLE_MS);

    expect(find.applied).toBe("gamma");
    expect(find.hits).toHaveLength(1);
  });

  it("walks the hits and wraps round", () => {
    const find = search();
    find.setQuery("beta");
    vi.advanceTimersByTime(SETTLE_MS);
    const seen: number[] = [];

    for (let step = 0; step < 4; step++) find.go(1, (index) => seen.push(index));

    expect(seen).toEqual([1, 2, 0, 1]);
  });

  it("walks backwards too", () => {
    const find = search();
    find.setQuery("beta");
    vi.advanceTimersByTime(SETTLE_MS);
    const seen: number[] = [];

    find.go(-1, (index) => seen.push(index));

    expect(seen).toEqual([2]);
  });

  it("does not move or reveal anything when there are no hits", () => {
    const find = search();
    find.setQuery("nowhere");
    vi.advanceTimersByTime(SETTLE_MS);
    const reveal = vi.fn();

    find.go(1, reveal);

    expect(reveal).not.toHaveBeenCalled();
    expect(find.at).toBe(0);
  });

  it("groups the spans of one cell together", () => {
    const find = search([["beta and beta", null]]);
    find.setQuery("beta");
    vi.advanceTimersByTime(SETTLE_MS);

    expect(find.spansFor(0, "left")).toEqual([
      [0, 4],
      [9, 13],
    ]);
    expect(find.spansFor(0, "right")).toEqual([]);
  });

  it("marks only the hit the counter points at", () => {
    const find = search([["beta and beta", null]]);
    find.setQuery("beta");
    vi.advanceTimersByTime(SETTLE_MS);

    expect(find.isCurrent(0, "left", 0)).toBe(true);
    expect(find.isCurrent(0, "left", 9)).toBe(false);

    find.go(1, () => {});
    expect(find.isCurrent(0, "left", 0)).toBe(false);
    expect(find.isCurrent(0, "left", 9)).toBe(true);
  });

  it("tells the two columns apart", () => {
    const find = search();
    find.setQuery("beta");
    vi.advanceTimersByTime(SETTLE_MS);

    expect(find.spansFor(1, "right")).toEqual([[0, 4]]);
    expect(find.spansFor(1, "left")).toEqual([]);
  });

  it("closing forgets the query and cancels a pending search", () => {
    const find = search();
    find.open();
    find.setQuery("beta");

    find.close();
    vi.advanceTimersByTime(SETTLE_MS * 2);

    expect(find.showing).toBe(false);
    expect(find.query).toBe("");
    expect(find.applied).toBe("");
    expect(find.hits).toHaveLength(0);
  });

  it("rewinds the counter without dropping the query", () => {
    const find = search();
    find.setQuery("beta");
    vi.advanceTimersByTime(SETTLE_MS);
    find.go(1, () => {});

    find.rewind();

    expect(find.at).toBe(0);
    expect(find.hits).toHaveLength(3);
  });
});
