import { describe, expect, it, vi } from "vitest";
import { eachAtMost } from "./fetch-all";
import { EMPTY_SELECTION, markRow } from "./multi-select";
import { fetchAllTargets } from "./repo-list";

const ORDER = ["C:/a", "C:/b", "C:/c"];
const OPEN = ORDER.map((root) => ({ root }));
const plain = { ctrl: false, shift: false };
const ctrl = { ctrl: true, shift: false };

const roots = (targets: readonly { root: string }[]) => targets.map((entry) => entry.root);

// F-140: a plain click on a row marked it, and Fetch All then fetched that one
// repository instead of every open one; after it was closed, nothing at all.
describe("what Fetch All fetches", () => {
  it("is every open repository after a plain click on a row", () => {
    const marked = markRow(EMPTY_SELECTION, "C:/b", ORDER, plain);

    expect(marked.paths.size).toBe(0);
    expect(roots(fetchAllTargets([...marked.paths], OPEN))).toEqual(ORDER);
  });

  it("is only the rows ticked with Ctrl, however many", () => {
    let marked = markRow(EMPTY_SELECTION, "C:/a", ORDER, ctrl);
    marked = markRow(marked, "C:/c", ORDER, ctrl);

    expect(roots(fetchAllTargets([...marked.paths], OPEN))).toEqual(["C:/a", "C:/c"]);
  });

  it("is a range picked with Shift from the row clicked last", () => {
    let marked = markRow(EMPTY_SELECTION, "C:/a", ORDER, plain);
    marked = markRow(marked, "C:/b", ORDER, { ctrl: false, shift: true });

    expect(roots(fetchAllTargets([...marked.paths], OPEN))).toEqual(["C:/a", "C:/b"]);
  });

  it("clears the ticks on a plain click", () => {
    const ticked = markRow(EMPTY_SELECTION, "C:/a", ORDER, ctrl);

    expect(markRow(ticked, "C:/b", ORDER, plain).paths.size).toBe(0);
  });

  it("is every open one again once the ticked ones are closed", () => {
    expect(roots(fetchAllTargets(["C:/gone"], OPEN))).toEqual(ORDER);
  });

  it("leaves out a ticked row that is not open", () => {
    expect(roots(fetchAllTargets(["C:/a", "C:/gone"], OPEN))).toEqual(["C:/a"]);
  });
});

// Fetch All went one repository at a time: a remote that did not answer held every other
// one until its timeout (M3-repo-tree.md, "Известные ловушки").
describe("eachAtMost", () => {
  it("runs a few at once, never more, and every one of them", async () => {
    let running = 0;
    let most = 0;
    const done: number[] = [];
    const gates = new Map<number, () => void>();
    const all = eachAtMost([1, 2, 3, 4, 5, 6], 4, async (item) => {
      running += 1;
      most = Math.max(most, running);
      await new Promise<void>((resolve) => gates.set(item, resolve));
      running -= 1;
      done.push(item);
    });

    await vi.waitFor(() => expect(gates.size).toBe(4));
    expect(most).toBe(4);
    for (let item = 1; item <= 6; item += 1) {
      await vi.waitFor(() => expect(gates.has(item)).toBe(true));
      gates.get(item)?.();
    }
    await all;

    expect(most).toBe(4);
    expect(done.sort()).toEqual([1, 2, 3, 4, 5, 6]);
  });

  it("does not let one failure stop the rest", async () => {
    const seen: number[] = [];
    await eachAtMost([1, 2, 3], 2, async (item) => {
      seen.push(item);
      if (item === 1) throw new Error("unreachable");
    }).catch(() => {});
    expect(seen.sort()).toEqual([1, 2, 3]);
  });
});
