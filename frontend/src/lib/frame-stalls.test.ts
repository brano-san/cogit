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

  it("splits the wait into input delay, handlers and the first frame, and says where", () => {
    expect(stallReport("wheel", [300], { delay: 4.2, handlers: 180.6, firstFrame: 300 }, "div.graph")).toBe(
      "after wheel: 1 frames in 300ms, median 300ms, worst 300ms, 1 over 50ms; input delay 4ms, handlers 181ms, first frame 300ms; on div.graph",
    );
  });
});
