import { describe, expect, it, vi } from "vitest";
import type { GraphOverlay, GraphPaintRequest, RepoId } from "$lib/ipc";

vi.mock("$lib/ipc", () => ({ graphOverlay: vi.fn() }));

const { GraphOverlayStore } = await import("./graph-overlay.svelte");

const REPO = 1 as RepoId;
const REQUEST: GraphPaintRequest = { tips: [{ oid: "a", slot: 2 }] };
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

/** Rust: one node style per row, the row number, so a test can tell windows apart. */
function rust(total: () => number) {
  const calls: { generation: number; start: number }[] = [];
  const fetch = vi.fn(
    async (
      _repo: RepoId,
      generation: number,
      start: number,
      count: number,
      _request?: GraphPaintRequest,
    ): Promise<GraphOverlay> => {
      calls.push({ generation, start });
      const rows = Math.max(Math.min(count, total() - start), 0);
      return {
        start,
        total: total(),
        nodeLanes: Array.from({ length: rows }, () => 0),
        nodeStyles: Array.from({ length: rows }, (_, i) => (start + i) % 16),
        segmentFirst: Array.from({ length: rows + 1 }, () => 0),
        segmentLanes: [],
        segmentStyles: [],
        folds: start === 0 ? [{ row: 2, hidden: 5 }] : [],
      };
    },
  );
  return { fetch, calls };
}

const view = (over: Partial<Parameters<InstanceType<typeof GraphOverlayStore>["show"]>[0]> = {}) => ({
  repo: REPO,
  generation: 1,
  start: 0,
  end: 40,
  total: 1000,
  complete: true,
  request: REQUEST,
  ...over,
});

describe("graph overlay", () => {
  it("asks for nothing while there is nothing to paint", async () => {
    const { fetch } = rust(() => 1000);
    const store = new GraphOverlayStore(fetch);
    store.show(view({ request: null }));
    await settle();
    expect(fetch).not.toHaveBeenCalled();
    expect(store.paintAt(3)).toBeUndefined();
  });

  it("fetches the blocks on screen once and paints their rows", async () => {
    const { fetch, calls } = rust(() => 1000);
    const store = new GraphOverlayStore(fetch);
    store.show(view({ start: 100, end: 140 }));
    await settle();
    store.show(view({ start: 101, end: 141 }));
    await settle();
    expect(calls.map((c) => c.start)).toEqual([0, 128]);
    expect(store.paintAt(130)?.nodeStyle).toBe(130 % 16);
  });

  it("starts over for another walk or another request", async () => {
    const { fetch, calls } = rust(() => 1000);
    const store = new GraphOverlayStore(fetch);
    store.show(view());
    await settle();
    store.show(view({ generation: 2 }));
    expect(store.paintAt(3)).toBeUndefined();
    await settle();
    store.show(view({ generation: 2, request: { tips: [] } }));
    await settle();
    expect(calls.map((c) => c.generation)).toEqual([1, 2, 2]);
  });

  it("drops an answer for a walk that has been replaced meanwhile", async () => {
    const { fetch } = rust(() => 1000);
    const store = new GraphOverlayStore(fetch);
    store.show(view());
    store.show(view({ generation: 2, request: null }));
    await settle();
    expect(store.paintAt(3)).toBeUndefined();
  });

  it("repaints rows on screen as the walk goes on, no more often than the refresh", async () => {
    vi.useFakeTimers();
    try {
      let total = 100;
      let now = 0;
      const { calls, fetch } = rust(() => total);
      const store = new GraphOverlayStore(fetch, () => now);
      store.show(view({ total, complete: false }));
      await vi.advanceTimersByTimeAsync(0);
      expect(calls).toHaveLength(1);

      total = 200;
      now = 100;
      store.show(view({ total, complete: false }));
      await vi.advanceTimersByTimeAsync(0);
      expect(calls).toHaveLength(1);

      now = 300;
      await vi.advanceTimersByTimeAsync(200);
      expect(calls).toHaveLength(2);

      store.show(view({ total, complete: true }));
      await vi.advanceTimersByTimeAsync(0);
      expect(calls).toHaveLength(2);
    } finally {
      vi.useRealTimers();
    }
  });
});

describe("folds", () => {
  it("say how many commits a merge row holds", async () => {
    const { fetch } = rust(() => 1000);
    const store = new GraphOverlayStore(fetch);
    store.show(view());
    await settle();
    expect(store.foldAt(2)).toBe(5);
    expect(store.foldAt(3)).toBe(0);
  });
});

describe("a new request for the same walk", () => {
  it("keeps the old paint on screen until the new one is in", async () => {
    let answer: (() => void) | null = null;
    const { fetch } = rust(() => 1000);
    const slow = vi.fn(async (...args: Parameters<typeof fetch>) => {
      if (args[4]?.ancestryOf) await new Promise<void>((resolve) => (answer = resolve));
      return fetch(...args);
    });
    const store = new GraphOverlayStore(slow);
    store.show(view());
    await settle();
    expect(store.paintAt(3)?.nodeStyle).toBe(3);

    store.show(view({ request: { ...REQUEST, ancestryOf: "b" } }));
    await settle();
    expect(store.paintAt(3)?.nodeStyle).toBe(3);
    answer!();
    await settle();
    expect(slow).toHaveBeenCalledTimes(2);

    store.show(view({ generation: 2 }));
    expect(store.paintAt(3)).toBeUndefined();
  });
});
