import { describe, expect, it } from "vitest";
import { trackTooltip } from "./repo-labels";

describe("trackTooltip", () => {
  it("says what ahead means and suggests a push", () => {
    expect(trackTooltip(2, 0)).toBe("Ahead: 2 commits not on the upstream yet. Push to publish them.");
  });

  it("says what behind means and suggests a pull", () => {
    expect(trackTooltip(0, 1)).toBe("Behind: 1 commit on the upstream not here yet. Pull to get them.");
  });

  it("calls both at once diverged, and says which comes first", () => {
    expect(trackTooltip(1, 3).split("\n")).toHaveLength(3);
    expect(trackTooltip(1, 3)).toMatch(/Diverged: pull .* before pushing/);
  });
});
