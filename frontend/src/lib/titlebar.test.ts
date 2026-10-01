import { describe, expect, it } from "vitest";
import { EDGES, edgeCursor, edgesFor, maximizeButton, startsDrag, topRow } from "./titlebar";

const idle = { maximized: false, fullscreen: false };
const own = { customTitlebar: true, webMenus: true };
const native = { customTitlebar: false, webMenus: false };

describe("topRow", () => {
  it("draws the titlebar when the window has no decorations", () => {
    expect(topRow(own, idle, true)).toBe("titlebar");
    expect(topRow({ customTitlebar: true, webMenus: false }, idle, false)).toBe("titlebar");
  });

  it("draws only the bar when the menus are the page but the decorations are native", () => {
    expect(topRow({ customTitlebar: false, webMenus: true }, idle, true)).toBe("menubar");
    expect(topRow({ customTitlebar: false, webMenus: true }, idle, false)).toBe("none");
  });

  it("draws nothing for a native window and in fullscreen", () => {
    expect(topRow(native, idle, true)).toBe("none");
    expect(topRow(own, { maximized: false, fullscreen: true }, true)).toBe("none");
  });
});

describe("maximizeButton", () => {
  it("offers Restore only while maximized", () => {
    expect(maximizeButton(idle)).toEqual({ title: "Maximize", glyph: "maximize" });
    expect(maximizeButton({ maximized: true, fullscreen: false })).toEqual({ title: "Restore", glyph: "restore" });
  });
});

describe("edgesFor", () => {
  it("has all eight grips on a free window and none otherwise", () => {
    expect(edgesFor(own, idle)).toEqual(EDGES);
    expect(edgesFor(own, { maximized: true, fullscreen: false })).toEqual([]);
    expect(edgesFor(own, { maximized: false, fullscreen: true })).toEqual([]);
    expect(edgesFor(native, idle)).toEqual([]);
  });

  it("names the Tauri resize directions", () => {
    expect([...EDGES].sort()).toEqual(
      ["East", "North", "NorthEast", "NorthWest", "South", "SouthEast", "SouthWest", "West"],
    );
  });
});

describe("edgeCursor", () => {
  it("points along the edge it drags", () => {
    expect(edgeCursor("North")).toBe("ns-resize");
    expect(edgeCursor("West")).toBe("ew-resize");
    expect(edgeCursor("NorthEast")).toBe("nesw-resize");
    expect(edgeCursor("SouthEast")).toBe("nwse-resize");
  });
});

describe("startsDrag", () => {
  const bar = { contains: () => true };
  const over = (selector: string | null) => ({
    button: 0,
    target: { closest: (asked: string) => (selector && asked.includes(selector) ? {} : null) },
  });

  it("drags from the empty bar with the primary button", () => {
    expect(startsDrag(over(null), bar)).toBe(true);
    expect(startsDrag({ ...over(null), button: 2 }, bar)).toBe(false);
  });

  it("leaves a button of the bar its click", () => {
    expect(startsDrag(over("button"), bar)).toBe(false);
  });
});
