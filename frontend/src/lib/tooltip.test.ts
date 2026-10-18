import { describe, expect, it } from "vitest";
import { STASH, placeTip, stashTitle } from "./tooltip";

const viewport = { width: 800, height: 600 };
const tip = { width: 100, height: 20 };

describe("placeTip", () => {
  it("sits centred above the anchor when there is room", () => {
    const at = placeTip({ left: 300, top: 300, width: 40, height: 20 }, tip, viewport);
    expect(at).toEqual({ left: 270, top: 274, below: false });
  });

  it("drops below an anchor at the top edge of the window", () => {
    const at = placeTip({ left: 300, top: 5, width: 40, height: 20 }, tip, viewport);
    expect(at.below).toBe(true);
    expect(at.top).toBe(31);
  });

  it("goes below when asked, for the toolbar under the title bar", () => {
    const at = placeTip({ left: 300, top: 300, width: 40, height: 20 }, tip, viewport, true);
    expect(at.below).toBe(true);
  });

  it("stays above when asked for below but the bottom edge is too close", () => {
    const at = placeTip({ left: 300, top: 570, width: 40, height: 20 }, tip, viewport, true);
    expect(at.below).toBe(false);
  });

  it("never runs off the left or the right edge", () => {
    expect(placeTip({ left: 0, top: 300, width: 10, height: 20 }, tip, viewport).left).toBe(4);
    expect(placeTip({ left: 790, top: 300, width: 10, height: 20 }, tip, viewport).left).toBe(696);
  });
});

/** The attributes of one element, as the DOM keeps them. */
function element(title: string | null) {
  const attrs = new Map<string, string>(title === null ? [] : [["title", title]]);
  return {
    attrs,
    getAttribute: (name: string) => attrs.get(name) ?? null,
    setAttribute: (name: string, value: string) => void attrs.set(name, value),
    removeAttribute: (name: string) => void attrs.delete(name),
  };
}

describe("stashTitle", () => {
  // A title still in the DOM when the pointer arrives is drawn by the browser too: a second,
  // native tooltip beside the app's own.
  it("moves a title aside the moment it appears, leaving none for the browser", () => {
    const button = element("Search for Repositories…");
    stashTitle(button);
    expect(button.getAttribute("title")).toBe("");
    expect(button.getAttribute(STASH)).toBe("Search for Repositories…");
  });

  it("follows a title its owner rewrites, and forgets one its owner removes", () => {
    const button = element("Opening…");
    stashTitle(button);
    button.setAttribute("title", "Open Repository…");
    stashTitle(button);
    expect(button.getAttribute(STASH)).toBe("Open Repository…");
    button.removeAttribute("title");
    stashTitle(button);
    expect(button.getAttribute(STASH)).toBeNull();
  });

  it("leaves its own empty title alone, so it does not loop on its own write", () => {
    const button = element("Tip");
    stashTitle(button);
    stashTitle(button);
    expect(button.getAttribute(STASH)).toBe("Tip");
  });
});
