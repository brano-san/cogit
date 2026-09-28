import { describe, expect, it } from "vitest";
import { upstreamStatus } from "./upstream-status";

describe("upstreamStatus", () => {
  it("names the remote for each way of standing", () => {
    expect(upstreamStatus("origin", 2, 1)).toBe("><origin");
    expect(upstreamStatus("origin", 2, 0)).toBe(">origin");
    expect(upstreamStatus("origin", 0, 3)).toBe("<origin");
  });

  it("says nothing when level or without an upstream", () => {
    expect(upstreamStatus("origin", 0, 0)).toBeUndefined();
    expect(upstreamStatus(null, 4, 4)).toBeUndefined();
  });
});
