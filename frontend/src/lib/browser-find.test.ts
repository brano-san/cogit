import { describe, expect, it, vi } from "vitest";
import { isBrowserFind, suppressBrowserFind } from "./browser-find";
import { ON_MAC } from "./platform";

const press = (key: string, ctrl = false, shift = false, alt = false, code?: string) => ({
  key,
  code,
  ctrlKey: ctrl,
  metaKey: false,
  shiftKey: shift,
  altKey: alt,
});

/** Ctrl, or ⌘ when the tests run on a Mac: what `suppressBrowserFind` reads as Ctrl. */
const primaryPress = (key: string) => ({ ...press(key), ctrlKey: !ON_MAC, metaKey: ON_MAC });

describe("isBrowserFind", () => {
  it("knows the keys the webview opens its own find bar on", () => {
    expect(isBrowserFind(press("f", true), false)).toBe(true);
    expect(isBrowserFind(press("F3"), false)).toBe(true);
    expect(isBrowserFind(press("F3", false, true), false)).toBe(true);
    expect(isBrowserFind(press("g", true), false)).toBe(true);
    expect(isBrowserFind(press("G", true, true), false)).toBe(true);
  });

  it("reads the key where F sits on another layout", () => {
    expect(isBrowserFind(press("а", true, false, false, "KeyF"), false)).toBe(true);
  });

  it("leaves every other key alone", () => {
    expect(isBrowserFind(press("f"), false)).toBe(false);
    expect(isBrowserFind(press("f", true, false, true), false)).toBe(false);
    expect(isBrowserFind(press("F6"), false)).toBe(false);
    expect(isBrowserFind(press("c", true), false)).toBe(false);
  });

  it("reads Win+F as no find off a Mac, and ⌘F as one on it", () => {
    expect(isBrowserFind({ ...press("f"), metaKey: true }, false)).toBe(false);
    expect(isBrowserFind({ ...press("f"), metaKey: true }, true)).toBe(true);
    expect(isBrowserFind(press("f", true), true)).toBe(false);
  });
});

describe("suppressBrowserFind", () => {
  function fakeWindow() {
    const listeners: ((event: KeyboardEvent) => void)[] = [];
    const win = {
      addEventListener: (_: string, fn: (event: KeyboardEvent) => void) => listeners.push(fn),
      removeEventListener: vi.fn(),
    } as unknown as Window;
    const fire = (over: object, handled = false) => {
      const event = {
        defaultPrevented: handled,
        preventDefault: vi.fn(),
        ...over,
      } as unknown as KeyboardEvent;
      for (const fn of listeners) fn(event);
      return event;
    };
    return { win, fire };
  }

  // Ctrl+F outside the Diff panel, or over a diff with no search, opened Edge's own find bar.
  it("keeps the webview's find bar shut when nothing in the page answered the key", () => {
    const { win, fire } = fakeWindow();
    suppressBrowserFind(win);

    expect(fire(primaryPress("f")).preventDefault).toHaveBeenCalled();
    expect(fire(primaryPress("x")).preventDefault).not.toHaveBeenCalled();
  });

  it("leaves a key the page already answered as it is", () => {
    const { win, fire } = fakeWindow();
    suppressBrowserFind(win);

    expect(fire(primaryPress("f"), true).preventDefault).not.toHaveBeenCalled();
  });
});
