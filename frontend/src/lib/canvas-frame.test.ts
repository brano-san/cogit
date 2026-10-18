import { describe, expect, it } from "vitest";
import { canvasBox, drawsNow, subpixelOffset } from "./canvas-frame";

describe("canvasBox", () => {
  it("shows the bitmap pixel for pixel, whatever the panel's fraction", () => {
    const cases: [number, number][] = [
      [401.6, 1.5],
      [333.3, 1.25],
      [500, 1.75],
      [240, 2],
    ];
    for (const [height, dpr] of cases) {
      const box = canvasBox(300, height, dpr);

      expect(box.pixelHeight / box.cssHeight).toBeCloseTo(dpr, 10);
      expect(box.pixelWidth / box.cssWidth).toBeCloseTo(dpr, 10);
    }
  });

  it("covers the whole panel", () => {
    const box = canvasBox(300.4, 401.6, 1.5);

    expect(box.cssHeight).toBeGreaterThanOrEqual(401.6);
    expect(box.cssWidth).toBeGreaterThanOrEqual(300.4);
  });

  it("treats a missing ratio as 1", () => {
    expect(canvasBox(300, 200, 0)).toEqual({ pixelWidth: 300, pixelHeight: 200, cssWidth: 300, cssHeight: 200 });
  });
});

describe("drawsNow", () => {
  const box = canvasBox(300, 400, 1.5);

  it("draws a panel that changed height in the same frame, not the next one", () => {
    expect(drawsNow(box, canvasBox(300, 404, 1.5))).toBe(true);
    expect(drawsNow(box, canvasBox(300, 396, 1.5))).toBe(true);
  });

  it("draws a new width or pixel ratio at once as well", () => {
    expect(drawsNow(box, canvasBox(320, 400, 1.5))).toBe(true);
    expect(drawsNow(box, canvasBox(300, 400, 2))).toBe(true);
  });

  it("draws the first picture at once", () => {
    expect(drawsNow(null, box)).toBe(true);
  });

  it("leaves a scroll or a hover to the next frame, where repeats collapse", () => {
    expect(drawsNow(box, canvasBox(300, 400, 1.5))).toBe(false);
  });
});

// A panel edge at 338 px is 422.5 device pixels at 125 %: the compositor resampled the
// whole bitmap half a pixel over, and every line came out blurred.
describe("subpixelOffset", () => {
  it("is the fraction of a device pixel the box starts past the grid", () => {
    expect(subpixelOffset(338, 75, 1.25)).toEqual({ x: 0.5, y: 0.75 });
  });

  it("is nothing on whole device pixels", () => {
    expect(subpixelOffset(338, 75, 1)).toEqual({ x: 0, y: 0 });
    expect(subpixelOffset(400, 80, 1.25)).toEqual({ x: 0, y: 0 });
  });

  it("takes a float a hair under a whole pixel for the pixel", () => {
    expect(subpixelOffset(0.1 * 3, 0, 10)).toEqual({ x: 0, y: 0 });
  });
});
