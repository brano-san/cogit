import { describe, expect, it, vi } from "vitest";
import { closesWindow, onMenuAction, whenCloses } from "./child-window";

const key = (k: string, mods: { ctrlKey?: boolean; metaKey?: boolean } = {}) => ({
  key: k,
  ctrlKey: mods.ctrlKey ?? false,
  metaKey: mods.metaKey ?? false,
});

describe("closesWindow", () => {
  it("closes on Escape", () => {
    expect(closesWindow(key("Escape"))).toBe(true);
  });

  it("closes on Ctrl+W", () => {
    expect(closesWindow(key("w", { ctrlKey: true }))).toBe(true);
  });

  it("closes on Cmd+W, for the day this runs on a Mac", () => {
    expect(closesWindow(key("w", { metaKey: true }))).toBe(true);
  });

  it("does not care which case the layout reports", () => {
    expect(closesWindow(key("W", { ctrlKey: true }))).toBe(true);
  });

  // The key was read as the character the layout types: on a Russian keyboard Ctrl+W
  // is "ц", and the compare and merge windows, which have no menu to catch it, stayed open.
  it("closes on Ctrl+W whatever the keyboard layout types there", () => {
    expect(closesWindow({ ...key("ц", { ctrlKey: true }), code: "KeyW" })).toBe(true);
  });

  it("leaves a bare W alone, which is a letter somebody is typing", () => {
    expect(closesWindow(key("w"))).toBe(false);
  });

  it("leaves everything else alone", () => {
    expect(closesWindow(key("Enter"))).toBe(false);
    expect(closesWindow(key("f", { ctrlKey: true }))).toBe(false);
  });
});

describe("whenCloses", () => {
  it("closes on Ctrl+W straight away: nothing inside the window wants it", () => {
    expect(whenCloses(key("w", { ctrlKey: true }))).toBe("now");
  });

  it("lets an open find bar take Escape first", () => {
    expect(whenCloses(key("Escape"))).toBe("unless-handled");
  });

  it("has nothing to do with other keys", () => {
    expect(whenCloses(key("Tab"))).toBeNull();
    expect(whenCloses(key("F6"))).toBeNull();
  });
});

describe("onMenuAction", () => {
  const menu = (detail: unknown) => new CustomEvent("cogit-menu", { detail });

  it("hands over the action Rust dispatched into this window", () => {
    const target = new EventTarget();
    const handler = vi.fn();
    onMenuAction(target, handler);

    target.dispatchEvent(menu("refresh"));

    expect(handler).toHaveBeenCalledWith("refresh");
  });

  it("ignores an event that carries no action", () => {
    const target = new EventTarget();
    const handler = vi.fn();
    onMenuAction(target, handler);

    target.dispatchEvent(menu(42));

    expect(handler).not.toHaveBeenCalled();
  });

  it("stops listening when undone", () => {
    const target = new EventTarget();
    const handler = vi.fn();
    const stop = onMenuAction(target, handler);

    stop();
    target.dispatchEvent(menu("refresh"));

    expect(handler).not.toHaveBeenCalled();
  });
});
