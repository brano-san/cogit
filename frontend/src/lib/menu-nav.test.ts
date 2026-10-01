import { describe, expect, it } from "vitest";
import { hoverRow, mnemonics, navKey, openFocused, typeaheadTo, TypedWord, type MenuRow } from "./menu-nav";

const row = (id: string, extra: Partial<MenuRow> = {}): MenuRow => ({
  id,
  label: id,
  enabled: true,
  separator: false,
  shortcut: null,
  checked: null,
  children: [],
  ...extra,
});
const line = row("", { separator: true, enabled: false });

const menu = [
  row("fetch"),
  row("pull", { enabled: false }),
  line,
  row("more", { children: [row("a"), row("b", { enabled: false }), row("c")] }),
  row("push"),
];

describe("navKey", () => {
  it("steps over rows that are off and separators, and goes round", () => {
    expect(navKey(menu, { path: [0] }, "ArrowDown")).toEqual({ kind: "state", nav: { path: [3] } });
    expect(navKey(menu, { path: [4] }, "ArrowDown")).toEqual({ kind: "state", nav: { path: [0] } });
    expect(navKey(menu, { path: [0] }, "ArrowUp")).toEqual({ kind: "state", nav: { path: [4] } });
  });

  it("starts at the first row from nothing", () => {
    expect(navKey(menu, { path: [-1] }, "ArrowDown")).toEqual({ kind: "state", nav: { path: [0] } });
  });

  it("opens a submenu with the right arrow and focuses its first live row", () => {
    expect(navKey(menu, { path: [3] }, "ArrowRight")).toEqual({ kind: "state", nav: { path: [3, 0] } });
  });

  it("closes only the submenu with Escape or the left arrow, then the menu", () => {
    expect(navKey(menu, { path: [3, 2] }, "Escape")).toEqual({ kind: "state", nav: { path: [3] } });
    expect(navKey(menu, { path: [3, 2] }, "ArrowLeft")).toEqual({ kind: "state", nav: { path: [3] } });
    expect(navKey(menu, { path: [3] }, "Escape")).toEqual({ kind: "close" });
  });

  it("activates the focused row with Enter and Space, never one that is off", () => {
    expect(navKey(menu, { path: [0] }, "Enter")).toEqual({ kind: "activate", row: menu[0] });
    expect(navKey(menu, { path: [0] }, " ")).toEqual({ kind: "activate", row: menu[0] });
    expect(navKey(menu, { path: [1] }, "Enter")).toEqual({ kind: "none" });
  });

  it("opens a submenu with Enter instead of activating it", () => {
    expect(navKey(menu, { path: [3] }, "Enter")).toEqual({ kind: "state", nav: { path: [3, 0] } });
  });

  it("hands the sideways arrows to the bar at the top level only", () => {
    expect(navKey(menu, { path: [0] }, "ArrowRight", true)).toEqual({ kind: "bar", step: 1 });
    expect(navKey(menu, { path: [0] }, "ArrowLeft", true)).toEqual({ kind: "bar", step: -1 });
    expect(navKey(menu, { path: [0] }, "ArrowRight")).toEqual({ kind: "none" });
    expect(navKey(menu, { path: [3, 0] }, "ArrowRight", true)).toEqual({ kind: "none" });
  });

  it("closes on Tab", () => {
    expect(navKey(menu, { path: [0] }, "Tab")).toEqual({ kind: "close" });
  });

  it("does nothing for a menu with no live row", () => {
    expect(navKey([line, row("x", { enabled: false })], { path: [-1] }, "ArrowDown")).toEqual({ kind: "none" });
  });
});

describe("pointer", () => {
  it("focuses the hovered row and closes what was open under the level", () => {
    expect(hoverRow({ path: [3, 2] }, 0, 4)).toEqual({ path: [4] });
    expect(hoverRow({ path: [3] }, 1, 2)).toEqual({ path: [3, 2] });
  });

  it("opens the focused row submenu with nothing focused inside", () => {
    expect(openFocused(menu, { path: [3] })).toEqual({ path: [3, -1] });
    expect(openFocused(menu, { path: [0] })).toEqual({ path: [0] });
  });
});

describe("typeaheadTo", () => {
  const rows = [row("Fetch"), row("Pull"), row("Push"), row("Prune", { enabled: false })];

  it("jumps to the next row starting with the letter and goes round", () => {
    expect(typeaheadTo(rows, 0, "p")).toBe(1);
    expect(typeaheadTo(rows, 1, "p")).toBe(2);
    expect(typeaheadTo(rows, 2, "p")).toBe(1);
  });

  it("narrows on a longer word and skips rows that are off", () => {
    expect(typeaheadTo(rows, 1, "pus")).toBe(2);
    expect(typeaheadTo(rows, 0, "pr")).toBeNull();
  });

  it("starts from the top when nothing is focused", () => {
    expect(typeaheadTo(rows, -1, "f")).toBe(0);
  });

  it("has no answer for nothing typed", () => {
    expect(typeaheadTo(rows, 0, "")).toBeNull();
  });
});

describe("TypedWord", () => {
  it("joins letters typed in quick succession and starts over after a pause", () => {
    const word = new TypedWord(1000);
    expect(word.add("p", 0)).toBe("p");
    expect(word.add("u", 500)).toBe("pu");
    expect(word.add("l", 2000)).toBe("l");
  });
});

describe("mnemonics", () => {
  it("takes the first letter nobody took before", () => {
    const titles = ["Repository", "Edit", "View", "Remote", "Local", "Branch", "Query", "Tools", "Window", "Help"];
    const picked = mnemonics(titles).map((at, index) => (at === null ? null : titles[index]![at]!.toLowerCase()));
    expect(picked).toEqual(["r", "e", "v", "m", "l", "b", "q", "t", "w", "h"]);
  });

  it("gives nothing to a title with no free letter", () => {
    expect(mnemonics(["A", "a"])).toEqual([0, null]);
  });
});
