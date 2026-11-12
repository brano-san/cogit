import { describe, expect, it, vi } from "vitest";
import { glideStep, nextTarget, wheelGlide, wheelPixels } from "./wheel-glide";

describe("wheelPixels", () => {
  it("turns lines and pages into pixels", () => {
    expect(wheelPixels(3, 1, 22, 400)).toBe(66);
    expect(wheelPixels(1, 2, 22, 400)).toBe(400);
    expect(wheelPixels(100, 0, 22, 400)).toBe(100);
  });
});

describe("nextTarget", () => {
  it("stops at the end of a list that overflows by a row or two", () => {
    expect(nextTarget(0, null, 100, 30)).toBe(30);
    expect(nextTarget(30, null, -100, 30)).toBe(0);
  });

  it("adds a notch to the aim, not to where the list is mid-glide", () => {
    expect(nextTarget(40, 100, 100, 1000)).toBe(200);
  });
});

describe("glideStep", () => {
  // The complaint: 30 px of overflow took as long to cross as a whole page.
  it("crosses a short and a long distance in the same time", () => {
    const frames = (distance: number) => {
      let at = 0;
      let n = 0;
      while (at !== distance && n < 1000) {
        at = glideStep(at, distance, 16);
        n++;
      }
      return n;
    };
    expect(frames(30)).toBeLessThanOrEqual(frames(300));
    expect(frames(300) * 16).toBeLessThan(250);
  });

  it("moves a good part of the way on the first frame", () => {
    expect(glideStep(0, 30, 16)).toBeGreaterThan(10);
  });

  it("ends on the target", () => {
    expect(glideStep(29.7, 30, 16)).toBe(30);
  });
});

describe("wheelGlide", () => {
  // The element keeps whole pixels: reading the position back stalled the glide a pixel short.
  it("reaches the end of a list that rounds its scroll position", () => {
    const frames: FrameRequestCallback[] = [];
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => frames.push(cb));
    vi.stubGlobal("cancelAnimationFrame", () => {});
    let top = 0;
    const el = {
      get scrollTop() {
        return top;
      },
      set scrollTop(v: number) {
        top = Math.round(Math.min(Math.max(v, 0), 30));
      },
      scrollLeft: 0,
      scrollHeight: 130,
      clientHeight: 100,
      clientWidth: 300,
    } as unknown as HTMLElement;
    const glide = wheelGlide(() => el, () => 22);

    glide.onwheel({ deltaY: 100, deltaX: 0, deltaMode: 0, preventDefault() {} } as WheelEvent);
    let now = performance.now();
    for (let i = 0; i < 100 && frames.length > 0; i++) frames.shift()!((now += 16));

    expect(top).toBe(30);
    expect(frames).toHaveLength(0);
    vi.unstubAllGlobals();
  });
});
