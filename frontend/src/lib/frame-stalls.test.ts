import { describe, expect, it } from "vitest";
import { stallReport } from "./frame-stalls";

describe("stallReport", () => {
  it("stays quiet while frames keep pace and none waits 50ms", () => {
    expect(stallReport("wheel", [8, 8, 9, 33, 49])).toBeNull();
    expect(stallReport("wheel", [])).toBeNull();
  });

  it("reports a steady rate below 50 frames a second", () => {
    expect(stallReport("wheel", [25, 25, 25])).toBe("after wheel: 3 frames in 75ms, median 25ms, worst 25ms, 0 over 50ms");
  });

  it("names the input, the median, the worst gap and the slow frames", () => {
    expect(stallReport("pointerdown", [16, 16, 60, 250, 16])).toBe(
      "after pointerdown: 5 frames in 358ms, median 16ms, worst 250ms, 2 over 50ms",
    );
  });

  it("splits the wait into input delay, handlers, the first frame and the work of each frame, and says where", () => {
    expect(stallReport("wheel", [300], { delay: 4.2, handlers: 180.6, firstFrame: 300, busy: [3, 9, 4] }, "div.graph")).toBe(
      "after wheel: 1 frames in 300ms, median 300ms, worst 300ms, 1 over 50ms; input delay 4ms, handlers 181ms, first frame 300ms, frame work median 4ms worst 9ms; on div.graph",
    );
  });
});
