import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, merge, needsRestart } from "./settings";

describe("merge", () => {
  it("returns the defaults for nothing stored", () => {
    expect(merge(null)).toEqual(DEFAULT_SETTINGS);
  });

  it("keeps a stored value", () => {
    expect(merge({ contextLines: 8 }).contextLines).toBe(8);
  });

  it("fills in a key the stored settings do not have", () => {
    expect(merge({ contextLines: 8 }).dateFormat).toBe(DEFAULT_SETTINGS.dateFormat);
  });

  it("drops a key that is no longer a setting", () => {
    expect(merge({ removedLongAgo: 1 } as never)).not.toHaveProperty("removedLongAgo");
  });

  it("falls back for a value of the wrong type", () => {
    expect(merge({ contextLines: "lots" } as never).contextLines).toBe(
      DEFAULT_SETTINGS.contextLines,
    );
  });

  it("clamps a context size that would make the diff unreadable", () => {
    expect(merge({ contextLines: -5 }).contextLines).toBe(0);
    expect(merge({ contextLines: 9999 }).contextLines).toBe(50);
  });

  it("clamps the lane width to something drawable", () => {
    expect(merge({ laneWidth: 1 }).laneWidth).toBeGreaterThanOrEqual(8);
    expect(merge({ laneWidth: 500 }).laneWidth).toBeLessThanOrEqual(40);
  });

  it("rejects an unknown enum value", () => {
    expect(merge({ pullMode: "rebase-and-pray" } as never).pullMode).toBe(
      DEFAULT_SETTINGS.pullMode,
    );
  });

  it("survives stored settings that are not an object at all", () => {
    expect(merge("nonsense" as never)).toEqual(DEFAULT_SETTINGS);
  });
});

describe("needsRestart", () => {
  it("is false for settings that apply immediately", () => {
    expect(needsRestart("contextLines")).toBe(false);
    expect(needsRestart("laneWidth")).toBe(false);
  });

  it("is true for the log level, which is read once at startup", () => {
    expect(needsRestart("logLevel")).toBe(true);
  });

  it("is true for the git executable path", () => {
    expect(needsRestart("gitPath")).toBe(true);
  });
});
