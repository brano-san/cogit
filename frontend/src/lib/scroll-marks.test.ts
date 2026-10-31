import { describe, expect, it } from "vitest";
import { rowMarks } from "./scroll-marks";

describe("rowMarks", () => {
  it("gives one mark per run of rows in one tone, placed on the scrollbar", () => {
    const tones = ["", "add", "add", "", "del", "changed", ""];
    expect(rowMarks(tones, 10, 70)).toEqual([
      { id: 1, top: 10, height: 20, tone: "add" },
      { id: 4, top: 40, height: 10, tone: "del" },
      { id: 5, top: 50, height: 10, tone: "changed" },
    ]);
  });

  it("squeezes a long file into the viewport and keeps a thin change visible", () => {
    const tones = Array.from({ length: 1000 }, (_, at) => (at === 500 ? "add" : ""));
    expect(rowMarks(tones, 18, 100)).toEqual([{ id: 500, top: 50, height: 3, tone: "add" }]);
  });
});
