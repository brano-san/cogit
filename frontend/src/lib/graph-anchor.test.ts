import { describe, expect, it } from "vitest";
import { anchoredScrollTop } from "./graph-anchor";

describe("anchoredScrollTop", () => {
  const rows = 1000;

  it("keeps the first visible row when the panel gets shorter", () => {
    expect(anchoredScrollTop({ scrollTop: 240, rowHeight: 24 }, { rowHeight: 24, viewportHeight: 300, totalRows: rows })).toBe(240);
  });

  it("keeps it when the panel gets taller, as long as the rows below fill it", () => {
    expect(anchoredScrollTop({ scrollTop: 240, rowHeight: 24 }, { rowHeight: 24, viewportHeight: 900, totalRows: rows })).toBe(240);
  });

  it("keeps the part of the first row that was scrolled off", () => {
    expect(anchoredScrollTop({ scrollTop: 250, rowHeight: 24 }, { rowHeight: 24, viewportHeight: 500, totalRows: rows })).toBe(250);
  });

  it("gives way at the end of the history, where nothing is left to fill the panel", () => {
    const bottom = rows * 24 - 400;

    expect(anchoredScrollTop({ scrollTop: bottom, rowHeight: 24 }, { rowHeight: 24, viewportHeight: 600, totalRows: rows })).toBe(
      rows * 24 - 600,
    );
  });

  it("keeps the same row first when the rows change height", () => {
    const at = anchoredScrollTop({ scrollTop: 24 * 100 + 12, rowHeight: 24 }, { rowHeight: 20, viewportHeight: 500, totalRows: rows });

    expect(Math.floor(at / 20)).toBe(100);
    expect(at - 100 * 20).toBe(10);
  });

  it("stays at the top of a history shorter than the panel", () => {
    expect(anchoredScrollTop({ scrollTop: 0, rowHeight: 24 }, { rowHeight: 24, viewportHeight: 900, totalRows: 5 })).toBe(0);
  });
});
