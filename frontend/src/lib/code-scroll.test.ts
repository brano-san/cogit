import { describe, expect, it } from "vitest";
import { clampOffset, maxOffset, revealOffset, textColumns, wheelSideways } from "./code-scroll";

describe("textColumns", () => {
  it("counts plain text by its length", () => {
    expect(textColumns("let x = 1;")).toBe(10);
    expect(textColumns("")).toBe(0);
    expect(textColumns("привет")).toBe(6);
  });

  it("runs a tab to the next stop of eight", () => {
    expect(textColumns("\tx")).toBe(9);
    expect(textColumns("ab\tx")).toBe(9);
    expect(textColumns("abcdefgh\tx")).toBe(17);
  });

  it("gives a wide character two columns and a combining mark none", () => {
    expect(textColumns("漢字")).toBe(4);
    expect(textColumns("é")).toBe(1);
    expect(textColumns("a😀")).toBe(3);
  });
});

describe("maxOffset", () => {
  it("is how far the widest line reaches past the column", () => {
    expect(maxOffset(150, 7.5, 400)).toBe(725);
  });

  it("is zero when every line fits or nothing is measured yet", () => {
    expect(maxOffset(40, 7.5, 400)).toBe(0);
    expect(maxOffset(150, 0, 400)).toBe(0);
    expect(maxOffset(150, 7.5, 0)).toBe(0);
  });
});

describe("clampOffset", () => {
  it("keeps the offset between the start and the end", () => {
    expect(clampOffset(-5, 100)).toBe(0);
    expect(clampOffset(50, 100)).toBe(50);
    expect(clampOffset(150, 100)).toBe(100);
    expect(clampOffset(20, 0)).toBe(0);
  });
});

describe("revealOffset (DF-037)", () => {
  const view = 400;
  const max = 1000;

  it("leaves a hit that is already in view where it is", () => {
    expect(revealOffset(0, view, 100, 130, max, 30)).toBe(0);
    expect(revealOffset(200, view, 200, 600, max, 30)).toBe(200);
  });

  it("scrolls right just far enough to show a hit past the right edge", () => {
    expect(revealOffset(0, view, 900, 930, max, 30)).toBe(560);
  });

  it("scrolls back to a hit left of the view", () => {
    expect(revealOffset(600, view, 100, 130, max, 30)).toBe(70);
  });

  it("shows the start of a hit wider than the column", () => {
    expect(revealOffset(0, view, 500, 1000, max, 30)).toBe(470);
  });

  it("never scrolls past either end", () => {
    expect(revealOffset(600, view, 10, 40, max, 30)).toBe(0);
    expect(revealOffset(0, view, 1380, 1400, max, 30)).toBe(max);
  });
});

describe("wheelSideways", () => {
  const wheel = (deltaX: number, deltaY: number, shiftKey = false, deltaMode = 0) => ({
    deltaX,
    deltaY,
    shiftKey,
    deltaMode,
  });

  it("takes a sideways swipe or a tilt", () => {
    expect(wheelSideways(wheel(40, 3))).toBe(40);
  });

  it("takes Shift with the wheel as sideways", () => {
    expect(wheelSideways(wheel(0, 100, true))).toBe(100);
  });

  it("leaves a vertical scroll alone", () => {
    expect(wheelSideways(wheel(0, 100))).toBe(0);
    expect(wheelSideways(wheel(2, 100))).toBe(0);
  });

  it("turns lines into pixels", () => {
    expect(wheelSideways(wheel(3, 0, false, 1), 18)).toBe(54);
  });
});
