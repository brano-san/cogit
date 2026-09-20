import { describe, expect, it } from "vitest";
import { GRAPH, edgeBand, indexByRow, visibleRange } from "./graph-geometry";

/** Per-frame cost only: it must not grow with history size. Real FPS is not measured. */

const ROWS = 50_000;
const VIEWPORT = 800;
const FRAME_BUDGET_MS = 16.6;

interface Edge {
  fromRow: number;
  toRow: number;
  fromLane: number;
  toLane: number;
}

function history(rows: number): Edge[] {
  const edges: Edge[] = [];
  for (let row = 1; row < rows; row++) {
    edges.push({ fromRow: row - 1, toRow: row, fromLane: row % 6, toLane: (row + 1) % 6 });
  }
  return edges;
}

describe("scrolling a 50 000 row history", () => {
  const edges = history(ROWS);

  it("indexes every edge once, not once per frame", () => {
    const started = performance.now();
    const index = indexByRow(edges);
    const elapsed = performance.now() - started;

    expect(index.size).toBe(ROWS - 1);
    expect(elapsed).toBeLessThan(250);
  });

  it("keeps the per-frame edge lookup independent of history size", () => {
    const index = indexByRow(edges);
    const small = indexByRow(history(500));
    const rowsOnScreen = Math.ceil(VIEWPORT / GRAPH.rowHeight);

    const measure = (idx: Map<number, Edge[]>, from: number) => {
      const started = performance.now();
      for (let i = 0; i < 200; i++) edgeBand(idx, from, from + rowsOnScreen);
      return performance.now() - started;
    };

    const big = measure(index, 25_000);
    const tiny = measure(small, 200);

    expect(big).toBeLessThan(Math.max(tiny * 8, 20));
  });

  it("draws one frame well inside the 60 FPS budget at the far end of the history", () => {
    const index = indexByRow(edges);
    const rowsOnScreen = Math.ceil(VIEWPORT / GRAPH.rowHeight);
    const scrollTop = (ROWS - rowsOnScreen) * GRAPH.rowHeight;

    const started = performance.now();
    const range = visibleRange(scrollTop, VIEWPORT, GRAPH.rowHeight, ROWS, 10);
    const band = edgeBand(index, range.start, range.end);
    const elapsed = performance.now() - started;

    expect(range.end - range.start).toBeLessThan(rowsOnScreen + 30);
    expect(band.length).toBeLessThan(rowsOnScreen + 30);
    expect(elapsed).toBeLessThan(FRAME_BUDGET_MS);
  });

  it("renders a bounded number of rows however far down the user scrolls", () => {
    const widths = [0, ROWS / 2, ROWS - 1].map((row) => {
      const range = visibleRange(row * GRAPH.rowHeight, VIEWPORT, GRAPH.rowHeight, ROWS, 10);
      return range.end - range.start;
    });

    expect(Math.max(...widths)).toBeLessThan(Math.ceil(VIEWPORT / GRAPH.rowHeight) + 30);
  });
});
