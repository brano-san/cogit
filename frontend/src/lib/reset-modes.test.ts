import { describe, expect, it } from "vitest";
import { RESET_CHOICES, resetChoice } from "./reset-modes";

describe("RESET_CHOICES", () => {
  it("offers the five modes of git reset, mixed first", () => {
    expect(RESET_CHOICES.map((choice) => choice.mode)).toEqual(["mixed", "soft", "hard", "keep", "merge"]);
  });

  it("explains every mode", () => {
    for (const choice of RESET_CHOICES) expect(choice.explanation.length).toBeGreaterThan(30);
  });

  it("asks for confirmation only before a hard reset", () => {
    expect(RESET_CHOICES.filter((choice) => choice.destructive).map((choice) => choice.mode)).toEqual([
      "hard",
    ]);
  });

  it("finds a mode by name", () => {
    expect(resetChoice("keep").label).toBe("Keep");
  });
});
