import { describe, expect, it } from "vitest";
import { splitSelection } from "./selection";

describe("splitSelection", () => {
  it("splits a block's line keys into the two lists the backend expects", () => {
    expect(splitSelection(new Set(["d:2", "i:5", "i:6"]))).toEqual({ deletes: [2], inserts: [5, 6] });
  });
});
