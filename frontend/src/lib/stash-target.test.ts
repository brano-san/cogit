import { describe, expect, it } from "vitest";
import { stashTarget } from "./stash-target";

const entry = (index: number, oid: string, message: string) => ({ index, oid, message, timestamp: 0 });

// A stale list said stash@{0} was "old" while the graph label clicked was the new one.
describe("stashTarget", () => {
  const stale = [entry(0, "old", "WIP old")];

  it("takes the oid of the clicked node, not the one the list has under that number", () => {
    expect(stashTarget(stale, 0, "new")).toEqual({ index: 0, oid: "new", message: "" });
  });

  it("takes the message and the current number from the entry that has that oid", () => {
    const fresh = [entry(0, "new", "WIP new"), entry(1, "old", "WIP old")];
    expect(stashTarget(fresh, 0, "old")).toEqual({ index: 1, oid: "old", message: "WIP old" });
  });

  it("finds the entry by its number only when the click has no oid", () => {
    expect(stashTarget(stale, 0, null)).toEqual({ index: 0, oid: "old", message: "WIP old" });
    expect(stashTarget(stale, 5, undefined)).toBeNull();
  });
});
