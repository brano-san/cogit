import { describe, expect, it } from "vitest";
import {
  NO_ITEMS,
  clickItem,
  extendItems,
  inOrder,
  keepShown,
  rightClickItem,
  selectEvery,
  single,
} from "./item-selection";

const order = ["a", "b", "c", "d", "e"];
const plain = { ctrl: false, shift: false };
const ctrl = { ctrl: true, shift: false };
const shift = { ctrl: false, shift: true };
const ids = (s: { ids: ReadonlySet<string> }) => [...s.ids].sort();

describe("click", () => {
  it("selects one row and anchors on it", () => {
    const s = clickItem(NO_ITEMS, "b", order, plain);
    expect(ids(s)).toEqual(["b"]);
    expect(s.anchor).toBe("b");
    expect(s.cursor).toBe("b");
  });

  it("toggles with Ctrl and moves the anchor", () => {
    let s = clickItem(NO_ITEMS, "a", order, plain);
    s = clickItem(s, "c", order, ctrl);
    expect(ids(s)).toEqual(["a", "c"]);
    s = clickItem(s, "a", order, ctrl);
    expect(ids(s)).toEqual(["c"]);
    expect(s.anchor).toBe("a");
  });

  it("takes a range from the anchor with Shift, either way", () => {
    let s = clickItem(NO_ITEMS, "b", order, plain);
    s = clickItem(s, "d", order, shift);
    expect(ids(s)).toEqual(["b", "c", "d"]);
    expect(s.anchor).toBe("b");
    s = clickItem(s, "a", order, shift);
    expect(ids(s)).toEqual(["a", "b"]);
  });

  it("ignores a row that is not in the order", () => {
    const s = single("a");
    expect(clickItem(s, "zzz", order, plain)).toBe(s);
  });
});

describe("extend", () => {
  it("grows from the anchor with each arrow and shrinks back", () => {
    let s = single("b");
    s = extendItems(s, order, 1);
    s = extendItems(s, order, 1);
    expect(ids(s)).toEqual(["b", "c", "d"]);
    s = extendItems(s, order, -1);
    expect(ids(s)).toEqual(["b", "c"]);
    expect(s.anchor).toBe("b");
  });

  it("clamps at the ends", () => {
    const s = extendItems(single("d"), order, 99);
    expect(ids(s)).toEqual(["d", "e"]);
    expect(extendItems(single("b"), order, -99).cursor).toBe("a");
  });

  it("starts from the first row when nothing is selected", () => {
    expect(ids(extendItems(NO_ITEMS, order, 1))).toEqual(["a"]);
  });
});

describe("select every, right-click, order", () => {
  it("selects every visible row", () => {
    expect(ids(selectEvery(single("c"), ["x", "y"]))).toEqual(["x", "y"]);
    expect(selectEvery(single("c"), order).cursor).toBe("c");
  });

  it("keeps the group on a selected row and replaces it on another", () => {
    const group = clickItem(single("a"), "c", order, ctrl);
    expect(rightClickItem(group, "c")).toBe(group);
    expect(ids(rightClickItem(group, "d"))).toEqual(["d"]);
  });

  it("lists the selection in display order, not click order", () => {
    let s = single("d");
    s = clickItem(s, "a", order, ctrl);
    s = clickItem(s, "c", order, ctrl);
    expect(inOrder(s, order)).toEqual(["a", "c", "d"]);
  });

  it("drops rows that are no longer shown", () => {
    const s = clickItem(single("a"), "d", order, ctrl);
    const kept = keepShown(s, ["a", "b"]);
    expect(ids(kept)).toEqual(["a"]);
    expect(kept.cursor).toBeNull();
    expect(keepShown(s, order)).toBe(s);
  });
});
