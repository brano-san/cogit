import { describe, expect, it } from "vitest";
import { menuKey } from "./menu-keys";

// The toolbar's dropdowns opened from the keyboard and could only be closed with the mouse.
describe("menuKey", () => {
  const all = [true, true, true];

  it("closes the menu on Esc and on Tab", () => {
    expect(menuKey("Escape", 1, all)).toEqual({ kind: "close" });
    expect(menuKey("Tab", null, all)).toEqual({ kind: "close" });
  });

  it("walks the items with the arrows, round the ends", () => {
    expect(menuKey("ArrowDown", null, all)).toEqual({ kind: "focus", to: 0 });
    expect(menuKey("ArrowDown", 0, all)).toEqual({ kind: "focus", to: 1 });
    expect(menuKey("ArrowDown", 2, all)).toEqual({ kind: "focus", to: 0 });
    expect(menuKey("ArrowUp", null, all)).toEqual({ kind: "focus", to: 2 });
    expect(menuKey("ArrowUp", 0, all)).toEqual({ kind: "focus", to: 2 });
  });

  it("jumps to the ends with Home and End", () => {
    expect(menuKey("Home", 2, all)).toEqual({ kind: "focus", to: 0 });
    expect(menuKey("End", 0, all)).toEqual({ kind: "focus", to: 2 });
  });

  it("steps over the items that are off", () => {
    const some = [false, true, false, true];
    expect(menuKey("ArrowDown", null, some)).toEqual({ kind: "focus", to: 1 });
    expect(menuKey("ArrowDown", 1, some)).toEqual({ kind: "focus", to: 3 });
    expect(menuKey("ArrowDown", 3, some)).toEqual({ kind: "focus", to: 1 });
    expect(menuKey("Home", 3, some)).toEqual({ kind: "focus", to: 1 });
    expect(menuKey("End", 1, some)).toEqual({ kind: "focus", to: 3 });
  });

  it("has nowhere to go in a menu with everything off", () => {
    expect(menuKey("ArrowDown", null, [false, false])).toBeNull();
  });

  it("leaves every other key to the item", () => {
    expect(menuKey("Enter", 0, all)).toBeNull();
    expect(menuKey("a", 0, all)).toBeNull();
  });
});
