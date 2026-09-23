import { describe, expect, it } from "vitest";
import { MENUS, commandForKey, matchesShortcut, visibleLines } from "./menu";

const key = (key: string, mods: Partial<Record<"ctrl" | "shift" | "alt", boolean>> = {}) => ({
  key,
  ctrlKey: !!mods.ctrl,
  metaKey: false,
  shiftKey: !!mods.shift,
  altKey: !!mods.alt,
});

describe("Investigate menu", () => {
  it("has the six menus the window promises", () => {
    expect(MENUS.map((menu) => menu.label)).toEqual([
      "File",
      "Edit",
      "View",
      "Go To",
      "Window",
      "Help",
    ]);
  });

  it("never gives two items the same shortcut", () => {
    const shortcuts = MENUS.flatMap((menu) =>
      menu.items.flatMap((line) => (line !== "separator" && line.shortcut ? [line.shortcut] : [])),
    );
    expect(new Set(shortcuts).size).toBe(shortcuts.length);
  });

  it("matches modifiers exactly", () => {
    expect(matchesShortcut(key("C", { ctrl: true, shift: true }), "Ctrl+Shift+C")).toBe(true);
    expect(matchesShortcut(key("c", { ctrl: true }), "Ctrl+Shift+C")).toBe(false);
    expect(matchesShortcut(key("ArrowLeft", { alt: true }), "Alt+Left")).toBe(true);
    expect(matchesShortcut(key("F6", { shift: true }), "F6")).toBe(false);
  });

  it("turns keys into commands", () => {
    expect(commandForKey(key("ArrowLeft", { alt: true }))).toBe("back");
    expect(commandForKey(key("F6"))).toBe("nextChange");
    expect(commandForKey(key("F6", { shift: true }))).toBe("previousChange");
    expect(commandForKey(key("4", { ctrl: true }))).toBe("perspective:blameOrigins");
    expect(commandForKey(key("w", { ctrl: true }))).toBe("close");
    expect(commandForKey(key("x"))).toBeNull();
  });

  it("drops leading, trailing and doubled separators", () => {
    const item = { id: "back" as const, label: "Back" };
    expect(visibleLines(["separator", item, "separator", "separator", item, "separator"])).toEqual([
      item,
      "separator",
      item,
    ]);
  });
});
