import { beforeEach, describe, expect, it, vi } from "vitest";

const searches = vi.hoisted(() => [] as { text: string; answer: (v: unknown) => void; fail: (e: unknown) => void }[]);

vi.mock("$lib/ipc", () => ({
  findObject: vi.fn(
    (_repo: unknown, text: string) =>
      new Promise((answer, fail) => searches.push({ text, answer, fail })),
  ),
}));

const { finder } = await import("./finder.svelte");
const A = 1 as never;

beforeEach(async () => {
  searches.length = 0;
  await finder.run(null, "");
});

// Type "a" and erase it: the search for "a" was left without anyone to end it, and the
// dialog said "Searching…" until it was closed; its late failure was reported too.
describe("a search replaced before it answers", () => {
  it("does not leave the dialog searching once the query is empty", async () => {
    const searching = finder.run(A, "a");
    await finder.run(A, "");
    searches[0]?.answer([{ kind: "commit", label: "a", oid: "a" }]);
    await searching;

    expect(finder.busy).toBe(false);
    expect(finder.results).toEqual([]);
  });

  it("keeps its failure to itself", async () => {
    const older = finder.run(A, "a");
    const newer = finder.run(A, "ab");
    searches[0]?.fail(new Error("cancelled"));
    searches[1]?.answer([]);

    await expect(older).resolves.toBeUndefined();
    await newer;
  });
});
