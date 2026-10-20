import { describe, expect, it } from "vitest";
import { placePopup } from "./popup-place";

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
