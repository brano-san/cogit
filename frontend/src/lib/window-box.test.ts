import { describe, expect, it } from "vitest";
import { clampBox, defaultBox, MIN_BOX } from "./window-box";
import { visibleRange } from "./graph-geometry";

const screen = { width: 1600, height: 900 };

describe("defaultBox", () => {
  it("fits inside the viewport it is given", () => {
    const box = defaultBox(screen);
    expect(box.x + box.w).toBeLessThanOrEqual(screen.width);
    expect(box.y + box.h).toBeLessThanOrEqual(screen.height);
  });

  it("is at least as big as the output needs to stay readable", () => {
    const box = defaultBox(screen);
    expect(box.w).toBeGreaterThanOrEqual(MIN_BOX.w);
    expect(box.h).toBeGreaterThanOrEqual(MIN_BOX.h);
  });

  it("still returns something usable on a window smaller than the minimum", () => {
    const box = defaultBox({ width: 400, height: 300 });
    expect(box.w).toBe(400);
    expect(box.h).toBe(300);
    expect(box.x).toBe(0);
  });
});

describe("clampBox", () => {
  it("leaves a box that already fits exactly where it was put", () => {
    const box = { x: 100, y: 80, w: 700, h: 500 };
    expect(clampBox(box, screen)).toEqual(box);
  });

  it("pulls a box saved on a bigger screen back into view", () => {
    const box = clampBox({ x: 2400, y: 1400, w: 700, h: 500 }, screen);
    expect(box.x + box.w).toBeLessThanOrEqual(screen.width);
    expect(box.y + box.h).toBeLessThanOrEqual(screen.height);
  });

  it("never leaves the header off the top or the left, where it cannot be grabbed", () => {
    const box = clampBox({ x: -300, y: -200, w: 700, h: 500 }, screen);
    expect(box.x).toBe(0);
    expect(box.y).toBe(0);
  });

  it("refuses a size that would make the output unreadable", () => {
    const box = clampBox({ x: 0, y: 0, w: 120, h: 60 }, screen);
    expect(box.w).toBe(MIN_BOX.w);
    expect(box.h).toBe(MIN_BOX.h);
  });

  it("gives up the minimum before it gives up fitting on screen", () => {
    const box = clampBox({ x: 0, y: 0, w: 900, h: 700 }, { width: 500, height: 320 });
    expect(box.w).toBe(500);
    expect(box.h).toBe(320);
  });

  it("rounds to whole pixels, so a drag cannot blur the text", () => {
    const box = clampBox({ x: 10.4, y: 20.6, w: 700.5, h: 500.5 }, screen);
    expect(Object.values(box).every(Number.isInteger)).toBe(true);
  });
});

describe("what the output window asks the DOM for", () => {
  it("shows a screenful of a huge log, not the log", () => {
    // 20 000 lines at the window's row height. The point of the number is that it does
    // not grow with the log: this is what keeps a hook's test output off the heap.
    const rows = visibleRange(0, 600, 18, 20_000, 20);
    expect(rows.end - rows.start).toBeLessThan(120);
  });

  it("asks for the same number however long the log is", () => {
    const small = visibleRange(900 * 18, 600, 18, 20_000, 20);
    const huge = visibleRange(900 * 18, 600, 18, 2_000_000, 20);
    expect(huge.end - huge.start).toBe(small.end - small.start);
  });

  it("stays bounded at the end of the log, where a reader lands on a failure", () => {
    const bottom = visibleRange(19_400 * 18, 600, 18, 20_000, 20);
    expect(bottom.end - bottom.start).toBeLessThan(120);
  });
});
