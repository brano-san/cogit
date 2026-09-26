import { describe, expect, it, vi } from "vitest";
import { isBrowserFind, suppressBrowserFind } from "./browser-find";

const press = (key: string, ctrl = false, shift = false, alt = false, code?: string) => ({
  key,
  code,
  ctrlKey: ctrl,
  metaKey: false,
  shiftKey: shift,
  altKey: alt,
});

describe("isBrowserFind", () => {
  it("knows the keys the webview opens its own find bar on", () => {
    expect(isBrowserFind(press("f", true))).toBe(true);
    expect(isBrowserFind(press("F3"))).toBe(true);
    expect(isBrowserFind(press("F3", false, true))).toBe(true);
    expect(isBrowserFind(press("g", true))).toBe(true);
    expect(isBrowserFind(press("G", true, true))).toBe(true);
  });

  it("reads the key where F sits on another layout", () => {
    expect(isBrowserFind(press("а", true, false, false, "KeyF"))).toBe(true);
  });

  it("leaves every other key alone", () => {
    expect(isBrowserFind(press("f"))).toBe(false);
    expect(isBrowserFind(press("f", true, false, true))).toBe(false);
    expect(isBrowserFind(press("F6"))).toBe(false);
    expect(isBrowserFind(press("c", true))).toBe(false);
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

    expect(fire(press("f", true)).preventDefault).toHaveBeenCalled();
    expect(fire(press("x", true)).preventDefault).not.toHaveBeenCalled();
  });

  it("leaves a key the page already answered as it is", () => {
    const { win, fire } = fakeWindow();
    suppressBrowserFind(win);

    expect(fire(press("f", true), true).preventDefault).not.toHaveBeenCalled();
  });
});
