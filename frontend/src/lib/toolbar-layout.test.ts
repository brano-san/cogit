import { describe, expect, it } from "vitest";
import { DEFAULT_LAYOUT, SEPARATOR } from "./toolbar";
import {
  addEntry,
  hiddenActions,
  moveEntry,
  normalizeLayout,
  removeEntry,
  sameLayout,
} from "./toolbar-layout";

describe("normalizeLayout", () => {
  it("is the default when nothing was stored", () => {
    expect(normalizeLayout(undefined)).toEqual(DEFAULT_LAYOUT);
    expect(normalizeLayout({ pull: true })).toEqual(DEFAULT_LAYOUT);
  });

  it("keeps the stored order", () => {
    expect(normalizeLayout(["push", SEPARATOR, "pull"])).toEqual(["push", SEPARATOR, "pull"]);
  });

  it("drops unknown ids and repeated buttons", () => {
    expect(normalizeLayout(["pull", "teleport", "pull", 7, "push"])).toEqual(["pull", "push"]);
  });

  it("never leaves two separators in a row or one at either end", () => {
    expect(normalizeLayout([SEPARATOR, "pull", SEPARATOR, SEPARATOR, "push", SEPARATOR])).toEqual([
      "pull",
      SEPARATOR,
      "push",
    ]);
  });

  it("allows an empty toolbar", () => {
    expect(normalizeLayout([])).toEqual([]);
  });
});

describe("editing", () => {
  it("lists what the toolbar leaves out", () => {
    expect(hiddenActions(["pull", "push"]).map((action) => action.id)).not.toContain("pull");
    expect(hiddenActions(DEFAULT_LAYOUT)).toEqual([]);
  });

  it("adds after the selected entry, or at the end", () => {
    expect(addEntry(["pull", "push"], "sync", 0)).toEqual(["pull", "sync", "push"]);
    expect(addEntry(["pull", "push"], "sync", null)).toEqual(["pull", "push", "sync"]);
  });

  it("removes one entry", () => {
    expect(removeEntry(["pull", SEPARATOR, "push"], 1)).toEqual(["pull", "push"]);
  });

  it("moves an entry one step and stops at the ends", () => {
    expect(moveEntry(["pull", "push", "sync"], 1, -1)).toEqual(["push", "pull", "sync"]);
    expect(moveEntry(["pull", "push"], 0, -1)).toEqual(["pull", "push"]);
    expect(moveEntry(["pull", "push"], 1, 1)).toEqual(["pull", "push"]);
  });

  it("tells a changed layout from the same one", () => {
    expect(sameLayout(DEFAULT_LAYOUT, [...DEFAULT_LAYOUT])).toBe(true);
    expect(sameLayout(["pull"], ["push"])).toBe(false);
  });
});
