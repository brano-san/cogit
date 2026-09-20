import { describe, expect, it } from "vitest";
import { overlapLabel, overlapTooltip } from "./overlap";

describe("overlapLabel", () => {
  it("names each degree in words a reader can act on", () => {
    expect(overlapLabel("none")).toBe("—");
    expect(overlapLabel("slight")).toBe("slight");
    expect(overlapLabel("heavy")).toBe("heavy");
    expect(overlapLabel("same")).toBe("same files");
  });
});

describe("overlapTooltip", () => {
  it("says so plainly when nothing is shared", () => {
    expect(overlapTooltip([], 0)).toBe("No files in common with the selected commit.");
  });

  it("lists the shared paths", () => {
    expect(overlapTooltip(["a.rs", "b.rs"], 2)).toBe("Also touched: a.rs, b.rs");
  });

  it("says how many more there are when the list is capped", () => {
    const shown = Array.from({ length: 10 }, (_, i) => `f${i}.rs`);
    expect(overlapTooltip(shown, 14)).toContain("and 4 more");
  });

  it("does not say 'and 0 more' when the list is complete", () => {
    expect(overlapTooltip(["a.rs"], 1)).not.toContain("more");
  });

  it("uses the singular for one extra path", () => {
    const shown = Array.from({ length: 10 }, (_, i) => `f${i}.rs`);
    expect(overlapTooltip(shown, 11)).toContain("and 1 more");
  });
});
