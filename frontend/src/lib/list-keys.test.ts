import { describe, expect, it } from "vitest";
import { TypeAhead, findTyped, listKey, typedChar } from "./list-keys";

const press = (key: string, mods: { ctrl?: boolean; shift?: boolean; alt?: boolean } = {}) => ({
  key,
  ctrl: mods.ctrl ?? false,
  shift: mods.shift ?? false,
  alt: mods.alt ?? false,
});

// 11 §10: one keyboard for every list panel. Branches had no keys at all and Files only
// Space; a click on a row and then ↓ left the selection where it was.
describe("listKey", () => {
  it("moves one row with the arrows, from nothing to the first", () => {
    expect(listKey(press("ArrowDown"), 2, 10, 5)).toEqual({ kind: "move", to: 3, extend: false });
    expect(listKey(press("ArrowUp"), 2, 10, 5)).toEqual({ kind: "move", to: 1, extend: false });
    expect(listKey(press("ArrowDown"), null, 10, 5)).toEqual({ kind: "move", to: 0, extend: false });
  });

  it("goes to the ends and a page at a time", () => {
    expect(listKey(press("Home"), 4, 10, 5)).toMatchObject({ to: 0 });
    expect(listKey(press("End"), 4, 10, 5)).toMatchObject({ to: 9 });
    expect(listKey(press("PageDown"), 4, 10, 3)).toMatchObject({ to: 7 });
    expect(listKey(press("PageUp"), 4, 10, 3)).toMatchObject({ to: 1 });
  });

  it("extends with Shift", () => {
    expect(listKey(press("ArrowDown", { shift: true }), 2, 10, 5)).toEqual({ kind: "move", to: 3, extend: true });
  });

  it("folds and unfolds with the side arrows, and runs the row with Enter", () => {
    expect(listKey(press("ArrowLeft"), 1, 10, 5)).toEqual({ kind: "fold", open: false });
    expect(listKey(press("ArrowRight"), 1, 10, 5)).toEqual({ kind: "fold", open: true });
    expect(listKey(press("Enter"), 1, 10, 5)).toEqual({ kind: "activate" });
    expect(listKey(press("Enter"), null, 10, 5)).toBeNull();
  });

  it("leaves Ctrl and Alt chords to their owners, and an empty list alone", () => {
    expect(listKey(press("ArrowDown", { ctrl: true }), 1, 10, 5)).toBeNull();
    expect(listKey(press("ArrowDown", { alt: true }), 1, 10, 5)).toBeNull();
    expect(listKey(press("ArrowDown"), null, 0, 5)).toBeNull();
    expect(listKey(press("a"), 1, 10, 5)).toBeNull();
  });
});

describe("typing to find a row", () => {
  const labels = ["main", "master", "feature", "Merge-fix"];

  it("finds the first row starting with what was typed, ignoring case", () => {
    expect(findTyped(labels, null, "fe")).toBe(2);
    expect(findTyped(labels, null, "mer")).toBe(3);
  });

  it("keeps the current row while it still matches a longer prefix", () => {
    expect(findTyped(labels, 1, "ma")).toBe(1);
  });

  it("steps through the rows a repeated first letter matches, round the end", () => {
    expect(findTyped(labels, 0, "m")).toBe(1);
    expect(findTyped(labels, 3, "m")).toBe(0);
  });

  it("finds nothing when nothing matches", () => {
    expect(findTyped(labels, 0, "zz")).toBeNull();
  });

  it("gathers the letters typed in a run and starts again after a pause", () => {
    const typing = new TypeAhead(800);
    expect(typing.type("m", 0)).toBe("m");
    expect(typing.type("a", 300)).toBe("ma");
    expect(typing.type("f", 2000)).toBe("f");
  });

  it("takes a printable character alone, never Space or a chord", () => {
    expect(typedChar(press("m"))).toBe("m");
    expect(typedChar(press("M", { shift: true }))).toBe("M");
    expect(typedChar(press(" "))).toBeNull();
    expect(typedChar(press("m", { ctrl: true }))).toBeNull();
    expect(typedChar(press("ArrowDown"))).toBeNull();
  });
});
