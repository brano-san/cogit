import { describe, expect, it, vi } from "vitest";

const commands = {
  overlapWindow: vi.fn(async (_repo: number, base: string, window: string[]) => ({
    status: "ok",
    data: window.map((oid) => ({ oid, overlap: "heavy", isBase: oid === base, shared: [], sharedTotal: 0 })),
  })),
};
vi.mock("$lib/ipc/bindings", () => ({ commands }));

const { overlap } = await import("./overlap.svelte");
const REPO = 1 as import("$lib/ipc").RepoId;

// Back on Working Tree, the column still showed the last commit's overlaps and its "base".
describe("the Overlap column", () => {
  it("shows the rows of the commit selected, and none on the Working Tree", async () => {
    overlap.toggle();
    await overlap.load(REPO, "a", ["a", "b"]);

    expect(overlap.rowOf("b", "a")?.overlap).toBe("heavy");
    expect(overlap.rowOf("b", null)).toBeUndefined();
    expect(overlap.rowOf("b", "c"), "another commit, its rows not in yet").toBeUndefined();
    overlap.toggle();
  });
});
