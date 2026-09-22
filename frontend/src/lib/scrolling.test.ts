import { describe, expect, it } from "vitest";
import type { Segment } from "$lib/ipc";
import { GRAPH, segmentCurve, visibleRange } from "./graph-geometry";

/** Per-frame cost only: it must not grow with history size. Real FPS is not measured.
    Segments belong to their row, so a frame touches the rows on screen and nothing else. */

const ROWS = 50_000;
const VIEWPORT = 800;
const FRAME_BUDGET_MS = 16.6;

function history(rows: number): Segment[][] {
  return Array.from({ length: rows }, (_, row) => [
    { from: 0, to: 0, span: "through", primary: true, color: 0, arrow: false },
    { from: row % 4, to: (row + 1) % 4, span: "through", primary: false, color: 1, arrow: false },
  ]);
}

describe("scrolling a 50 000 row history", () => {
  const rows = history(ROWS);

  it("draws one frame well inside the 60 FPS budget at the far end of the history", () => {
    const rowsOnScreen = Math.ceil(VIEWPORT / GRAPH.rowHeight);
    const scrollTop = (ROWS - rowsOnScreen) * GRAPH.rowHeight;

    const started = performance.now();
    const range = visibleRange(scrollTop, VIEWPORT, GRAPH.rowHeight, ROWS, 10);
    let curves = 0;
    for (let row = range.start; row < range.end; row++) {
      for (const segment of rows[row]!) {
        segmentCurve(segment, row, scrollTop);
        curves += 1;
      }
    }
    const elapsed = performance.now() - started;

    expect(curves).toBeLessThan((rowsOnScreen + 30) * 2);
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
