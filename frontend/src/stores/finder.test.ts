import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const searches = vi.hoisted(() => [] as { text: string; answer: (v: unknown) => void; fail: (e: unknown) => void }[]);

vi.mock("$lib/ipc", () => ({
  findObject: vi.fn(
    (_repo: unknown, text: string) =>
      new Promise((answer, fail) => searches.push({ text, answer, fail })),
  ),
}));

const { finder, SETTLE_MS } = await import("./finder.svelte");
const A = 1 as never;

beforeEach(async () => {
  vi.useFakeTimers();
  searches.length = 0;
  await finder.run(null, "");
});

afterEach(() => {
  vi.useRealTimers();
});

// Type "a" and erase it: the search for "a" was left without anyone to end it, and the
// dialog said "Searching…" until it was closed; its late failure was reported too.
describe("a search replaced before it answers", () => {
  it("does not leave the dialog searching once the query is empty", async () => {
    const searching = finder.run(A, "a");
    await vi.advanceTimersByTimeAsync(SETTLE_MS);
    await finder.run(A, "");
    searches[0]?.answer([{ kind: "commit", label: "a", oid: "a" }]);
    await searching;

    expect(finder.busy).toBe(false);
    expect(finder.results).toEqual([]);
  });

  it("keeps its failure to itself", async () => {
    const older = finder.run(A, "a");
    await vi.advanceTimersByTimeAsync(SETTLE_MS);
    const newer = finder.run(A, "ab");
    await vi.advanceTimersByTimeAsync(SETTLE_MS);
    searches[0]?.fail(new Error("cancelled"));
    searches[1]?.answer([]);

    await expect(older).resolves.toBeUndefined();
    await newer;
    expect(searches.map((search) => search.text)).toEqual(["a", "ab"]);
  });
});

// Every letter started a walk of the whole history, and on a large repository the walks
// piled up; only their answers were dropped.
describe("typing", () => {
  it("searches once the typing pauses, for the last query", async () => {
    const runs = [..."abcdefghij"].map((_, at, letters) => finder.run(A, letters.slice(0, at + 1).join("")));
    await vi.advanceTimersByTimeAsync(SETTLE_MS);

    expect(searches.map((search) => search.text)).toEqual(["abcdefghij"]);
    searches[0]?.answer([]);
    await Promise.all(runs);
    expect(finder.busy).toBe(false);
  });

  it("lets a replaced query end without searching", async () => {
    const first = finder.run(A, "a");
    const second = finder.run(A, "ab");

    await expect(first).resolves.toBeUndefined();
    await vi.advanceTimersByTimeAsync(SETTLE_MS);
    searches[0]?.answer([]);
    await second;
    expect(searches).toHaveLength(1);
  });
});
