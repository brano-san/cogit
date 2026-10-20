import { describe, expect, it } from "vitest";
import { BASE_FONT_PX, scaled, uiFontPx, uiMetrics, zoomStep } from "./ui-metrics";

// Cogit's text read smaller than SmartGit's next to it, at a fixed 13px whatever the desktop.
describe("uiFontPx", () => {
  it("starts from the desktop's size, not below the one the layout was drawn at", () => {
    expect(uiFontPx(0, 11 * (4 / 3))).toBe(14.5);
    expect(uiFontPx(0, 12)).toBe(BASE_FONT_PX);
    expect(uiFontPx(0, null)).toBe(BASE_FONT_PX);
  });

  it("takes the size chosen in Preferences, within its range", () => {
    expect(uiFontPx(15, 12)).toBe(15);
    expect(uiFontPx(40, 12)).toBe(20);
  });
});

describe("uiMetrics", () => {
  it("keeps the rows the layout was drawn with at the base size", () => {
    const base = uiMetrics(BASE_FONT_PX, 12.5);
    expect([base.listRow, base.denseRow]).toEqual([24, 22]);
    expect(base.css["--fs-ui"]).toBe("13px");
    expect(base.css["--h-row"]).toBe("24px");
  });

  it("grows the rows with the font, so a larger font is never cut", () => {
    const large = uiMetrics(16, 13);
    expect(large.listRow).toBe(30);
    expect(large.denseRow).toBe(27);
    expect(large.css["--fs-dense"]).toBe("15px");
    expect(large.css["--fs-code"]).toBe("13px");
  });

  it("holds the diff font to what its 18 px rows take", () => {
    expect(uiMetrics(13, 20).css["--fs-code"]).toBe("14px");
  });

  it("scales a length drawn for the base font", () => {
    expect(scaled(28, 13)).toBe(28);
    expect(scaled(20, 16)).toBe(25);
  });
});

describe("zoomStep", () => {
  it("steps as a browser does, and resets to 100%", () => {
    expect(zoomStep(1, 1)).toBe(1.1);
    expect(zoomStep(1, -1)).toBe(0.9);
    expect(zoomStep(1.3, -1)).toBe(1.25);
    expect(zoomStep(2, 1)).toBe(2);
    expect(zoomStep(0.5, -1)).toBe(0.5);
    expect(zoomStep(1.75, 0)).toBe(1);
  });
});
