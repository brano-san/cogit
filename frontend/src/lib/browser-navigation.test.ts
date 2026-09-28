import { describe, expect, it } from "vitest";
import { isHistoryButton } from "./browser-navigation";

describe("isHistoryButton", () => {
  it("is the two side buttons only", () => {
    expect([0, 1, 2, 3, 4].filter(isHistoryButton)).toEqual([3, 4]);
  });
});
