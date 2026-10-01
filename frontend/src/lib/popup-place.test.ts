import { describe, expect, it } from "vitest";
import { placeAtPoint, placeDropdown, placePopup, placeSubmenu } from "./popup-place";

const view = { width: 800, height: 600 };
const at = (top: number) => ({ left: 700, right: 760, top, bottom: top + 20 });

describe("placePopup", () => {
  it("opens below, right-aligned", () => {
    expect(placePopup(at(100), { width: 200, height: 100 }, view)).toEqual({
      left: 560, top: 122, maxHeight: 474, above: false,
    });
  });

  it("flips above when there is more room there", () => {
    const p = placePopup(at(500), { width: 200, height: 300 }, view);
    expect(p.above).toBe(true);
    expect(p.top + Math.min(300, p.maxHeight)).toBe(498);
  });

  it("scrolls when neither side fits", () => {
    const p = placePopup(at(150), { width: 200, height: 900 }, view);
    expect(p.maxHeight).toBeLessThan(900);
    expect(p.top).toBeGreaterThanOrEqual(0);
  });

  it("stays inside the left and right edges", () => {
    expect(placePopup({ left: 0, right: 30, top: 10, bottom: 30 }, { width: 200, height: 50 }, view).left).toBe(4);
    expect(placePopup({ left: 790, right: 800, top: 10, bottom: 30 }, { width: 200, height: 50 }, view).left).toBe(596);
  });
});

describe("placeAtPoint", () => {
  const size = { width: 200, height: 100 };

  it("opens with its corner at the point", () => {
    expect(placeAtPoint({ x: 100, y: 120 }, size, view)).toEqual({ left: 100, top: 120, maxHeight: 476 });
  });

  it("flips to the left of the point at the right edge", () => {
    expect(placeAtPoint({ x: 700, y: 120 }, size, view).left).toBe(500);
  });

  it("flips above the point at the bottom edge when there is more room there", () => {
    const p = placeAtPoint({ x: 100, y: 560 }, size, view);
    expect(p.top).toBe(460);
  });

  it("scrolls, inside the window, when neither side fits", () => {
    const p = placeAtPoint({ x: 100, y: 300 }, { width: 200, height: 900 }, view);
    expect(p.top).toBeGreaterThanOrEqual(4);
    expect(p.maxHeight).toBeLessThanOrEqual(592);
  });

  it("never leaves the left edge", () => {
    expect(placeAtPoint({ x: 0, y: 0 }, { width: 900, height: 50 }, view).left).toBe(4);
  });
});

describe("placeSubmenu", () => {
  const row = { left: 100, right: 300, top: 200, bottom: 224 };
  const size = { width: 180, height: 120 };

  it("opens beside the row, level with it", () => {
    expect(placeSubmenu(row, size, view)).toMatchObject({ left: 299, top: 196, flipped: false });
  });

  it("flips to the left when the right side is too narrow", () => {
    const wide = { left: 520, right: 760, top: 200, bottom: 224 };
    expect(placeSubmenu(wide, size, view)).toMatchObject({ left: 521 - 180, flipped: true });
  });

  it("slides up to stay inside the window at the bottom", () => {
    const low = { left: 100, right: 300, top: 560, bottom: 584 };
    const p = placeSubmenu(low, size, view);
    expect(p.top + size.height).toBeLessThanOrEqual(596);
  });

  it("stays on the right when neither side fits but the right is wider", () => {
    const tiny = { width: 400, height: 600 };
    const p = placeSubmenu({ left: 10, right: 200, top: 10, bottom: 30 }, { width: 300, height: 50 }, tiny);
    expect(p.flipped).toBe(false);
  });
});

describe("placeDropdown", () => {
  const size = { width: 220, height: 300 };

  it("hangs from the left edge of its title, right under it", () => {
    expect(placeDropdown({ left: 60, right: 110, top: 0, bottom: 28 }, size, view)).toEqual({ left: 60, top: 28, maxHeight: 568 });
  });

  it("shifts left where the window ends", () => {
    expect(placeDropdown({ left: 760, right: 790, top: 0, bottom: 28 }, size, view).left).toBe(576);
  });

  it("scrolls rather than flipping when the menu is taller than the room", () => {
    const p = placeDropdown({ left: 0, right: 40, top: 0, bottom: 28 }, { width: 100, height: 900 }, view);
    expect(p.top).toBe(28);
    expect(p.maxHeight).toBe(568);
  });
});
