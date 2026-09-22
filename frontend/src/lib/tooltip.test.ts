import { describe, expect, it } from "vitest";
import { placeTip } from "./tooltip";

const viewport = { width: 800, height: 600 };
const tip = { width: 100, height: 20 };

describe("placeTip", () => {
  it("sits centred above the anchor when there is room", () => {
    const at = placeTip({ left: 300, top: 300, width: 40, height: 20 }, tip, viewport);
    expect(at).toEqual({ left: 270, top: 274, below: false });
  });

  it("drops below an anchor at the top edge of the window", () => {
    const at = placeTip({ left: 300, top: 5, width: 40, height: 20 }, tip, viewport);
    expect(at.below).toBe(true);
    expect(at.top).toBe(31);
  });

  it("goes below when asked, for the toolbar under the title bar", () => {
    const at = placeTip({ left: 300, top: 300, width: 40, height: 20 }, tip, viewport, true);
    expect(at.below).toBe(true);
  });

  it("stays above when asked for below but the bottom edge is too close", () => {
    const at = placeTip({ left: 300, top: 570, width: 40, height: 20 }, tip, viewport, true);
    expect(at.below).toBe(false);
  });

  it("never runs off the left or the right edge", () => {
    expect(placeTip({ left: 0, top: 300, width: 10, height: 20 }, tip, viewport).left).toBe(4);
    expect(placeTip({ left: 790, top: 300, width: 10, height: 20 }, tip, viewport).left).toBe(696);
  });
});
