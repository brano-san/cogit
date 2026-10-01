import { describe, expect, it } from "vitest";
import { decideSelectAll, isSelectAllKey, type SelectAllContext } from "./select-all";

const base: SelectAllContext = {
  editable: false,
  own: false,
  panel: "graph",
  lists: new Set(["repositories", "refs", "graph", "files"]),
  region: null,
};

describe("decideSelectAll", () => {
  it("leaves a field to the browser", () => {
    expect(decideSelectAll({ ...base, editable: true })).toBe("native");
    expect(decideSelectAll({ ...base, editable: true, region: { panel: "diff" } })).toBe("native");
  });

  it("leaves what answers for itself alone", () => {
    expect(decideSelectAll({ ...base, own: true })).toBe("own");
  });

  it("gives a list panel its Select All", () => {
    for (const panel of ["repositories", "refs", "graph", "files"]) {
      expect(decideSelectAll({ ...base, panel })).toBe("list");
    }
  });

  it("selects text only in the region of the panel in focus", () => {
    const diff = { ...base, panel: "diff", region: { panel: "diff" } };
    expect(decideSelectAll(diff)).toBe("text");
    expect(decideSelectAll({ ...diff, region: { panel: "commit" } })).toBe("none");
    expect(decideSelectAll({ ...diff, region: null })).toBe("none");
  });

  it("does nothing in a panel without multi-select", () => {
    expect(decideSelectAll({ ...base, panel: "worktrees" })).toBe("none");
    expect(decideSelectAll({ ...base, panel: "commit" })).toBe("none");
  });

  it("takes the region in a child window, whatever panel it names", () => {
    expect(decideSelectAll({ ...base, panel: null, region: { panel: null } })).toBe("text");
    expect(decideSelectAll({ ...base, panel: null })).toBe("none");
  });

  it("prefers the list over a region of the same panel", () => {
    expect(decideSelectAll({ ...base, region: { panel: "graph" } })).toBe("list");
  });
});

describe("isSelectAllKey", () => {
  const press = { key: "a", code: "KeyA", ctrlKey: true, metaKey: false, shiftKey: false, altKey: false };

  it("takes Ctrl+A and Cmd+A by place, so a Russian layout counts", () => {
    expect(isSelectAllKey(press)).toBe(true);
    expect(isSelectAllKey({ ...press, ctrlKey: false, metaKey: true })).toBe(true);
    expect(isSelectAllKey({ ...press, key: "ф" })).toBe(true);
  });

  it("refuses other chords", () => {
    expect(isSelectAllKey({ ...press, ctrlKey: false })).toBe(false);
    expect(isSelectAllKey({ ...press, shiftKey: true })).toBe(false);
    expect(isSelectAllKey({ ...press, code: "KeyB", key: "b" })).toBe(false);
  });
});
