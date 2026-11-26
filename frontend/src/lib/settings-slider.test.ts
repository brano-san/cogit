import { expect, it } from "vitest";
import { readSliderNumber } from "./settings-slider";

it("reads an integer without rounding typed values to slider shortcuts", () => {
  expect(readSliderNumber("  +007  ", 0, 60)).toBe(7);
  expect(readSliderNumber("100", 0, 200)).toBe(100);
});

it("clamps integers to either limit, including values larger than a Number", () => {
  expect(readSliderNumber("-10", 20, 80)).toBe(20);
  expect(readSliderNumber("81", 20, 80)).toBe(80);
  expect(readSliderNumber("9".repeat(400), 20, 80)).toBe(80);
});

it.each(["", "-", "words", "20%", "7.5", "2e4", "0x40"])("rejects %j instead of coercing it", (text) => {
  expect(readSliderNumber(text, 0, 100)).toBeNull();
});
