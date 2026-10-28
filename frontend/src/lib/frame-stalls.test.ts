import { describe, expect, it } from "vitest";
import { stallReport } from "./frame-stalls";

describe("stallReport", () => {
  it("stays quiet while every frame is in time", () => {
    expect(stallReport("wheel", [16, 17, 16, 33, 99])).toBeNull();
    expect(stallReport("wheel", [])).toBeNull();
  });

  it("names the input, the median, the worst gap and the slow frames", () => {
    expect(stallReport("pointerdown", [16, 16, 60, 250, 16])).toBe(
      "after pointerdown: 5 frames in 358ms, median 16ms, worst 250ms, 2 over 50ms",
    );
  });
});
