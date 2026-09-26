import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { affected, clear, mark, STALE_LINGER_MS, STALE_SHOW_MS, StaleDot } from "./staleness";
import type { PanelId } from "./perspectives";

describe("affected", () => {
  it("puts a ref move on the graph and the references", () => {
    expect(affected("refs")).toEqual(["refs", "graph"]);
  });

  it("puts an index change on the panels that show the index", () => {
    expect(affected("index")).toContain("files");
    expect(affected("index")).not.toContain("refs");
  });

  it("leaves the panels alone for a hook change, which none of them show", () => {
    expect(affected("hooks")).toEqual([]);
  });

  it("puts a .mailmap edit on the panels that name authors", () => {
    expect(affected("mailmap")).toEqual(["graph", "commit"]);
  });
});

describe("mark", () => {
  it("adds the panels a change touches", () => {
    expect([...mark(new Set(), "refs")].sort()).toEqual(["graph", "refs"]);
  });

  it("keeps what was already stale", () => {
    const before = new Set<PanelId>(["files"]);
    expect(mark(before, "refs").has("files")).toBe(true);
  });

  it("leaves the set it was given alone", () => {
    const before = new Set<PanelId>();
    mark(before, "refs");
    expect(before.size).toBe(0);
  });
});

describe("clear", () => {
  it("drops the panels that reloaded", () => {
    const stale = new Set<PanelId>(["graph", "files"]);
    expect([...clear(stale, ["graph"])]).toEqual(["files"]);
  });

  it("is quiet about a panel that was never stale", () => {
    expect([...clear(new Set<PanelId>(["graph"]), ["diff"])]).toEqual(["graph"]);
  });
});

// A folder written to every two seconds lit the dot of Files and Repositories and put it
// out a moment later, again and again (#35).
describe("the stale dot", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  function dot() {
    const seen: boolean[] = [];
    const flag = new StaleDot((shown) => seen.push(shown));
    return { flag, seen };
  }

  it("stays dark for a reload that is over quickly, however often it comes", () => {
    const { flag, seen } = dot();
    for (let second = 0; second < 10; second += 1) {
      flag.set(true);
      vi.advanceTimersByTime(200);
      flag.set(false);
      vi.advanceTimersByTime(1800);
    }
    expect(seen).toEqual([]);
  });

  it("lights for a reload that takes a while, and goes out once it is over", () => {
    const { flag, seen } = dot();
    flag.set(true);
    vi.advanceTimersByTime(STALE_SHOW_MS);
    expect(seen).toEqual([true]);
    flag.set(false);
    vi.advanceTimersByTime(STALE_LINGER_MS);
    expect(seen).toEqual([true, false]);
  });

  it("stays lit through the gaps between slow reloads rather than blinking", () => {
    const { flag, seen } = dot();
    for (let round = 0; round < 5; round += 1) {
      flag.set(true);
      vi.advanceTimersByTime(STALE_SHOW_MS + 400);
      flag.set(false);
      vi.advanceTimersByTime(STALE_LINGER_MS - 100);
    }
    expect(seen).toEqual([true]);
  });

  it("starts nothing once let go", () => {
    const { flag, seen } = dot();
    flag.set(true);
    flag.dispose();
    vi.advanceTimersByTime(STALE_SHOW_MS * 2);
    expect(seen).toEqual([]);
  });
});
