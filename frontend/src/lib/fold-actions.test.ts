import { describe, expect, it } from "vitest";
import { FOLD_HYSTERESIS_PX, foldDecision } from "./fold-actions";

describe("foldDecision", () => {
  it("folds only when the buttons do not fit", () => {
    expect(foldDecision(120, 120, false)).toBe(false);
    expect(foldDecision(119, 120, false)).toBe(true);
  });

  it("comes back only with room to spare, so the boundary does not flicker", () => {
    expect(foldDecision(120, 120, true)).toBe(true);
    expect(foldDecision(120 + FOLD_HYSTERESIS_PX - 1, 120, true)).toBe(true);
    expect(foldDecision(120 + FOLD_HYSTERESIS_PX, 120, true)).toBe(false);
  });
});
