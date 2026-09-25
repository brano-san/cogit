import { describe, expect, it, vi } from "vitest";
import { closeGuard, closesWindow, onMenuAction, whenCloses } from "./child-window";

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

// Every way out of a window — its ✕, Esc, Ctrl+W, a Cancel button — arrives as one close
// request; a window with work in it asks there, once.
describe("closeGuard", () => {
  const request = () => ({ prevented: false, preventDefault() { this.prevented = true; } });

  it("lets a window with nothing unsaved close without asking", async () => {
    const ask = vi.fn(async () => false);
    const closing = request();
    await closeGuard(() => false, ask)(closing);
    expect(ask).not.toHaveBeenCalled();
    expect(closing.prevented).toBe(false);
  });

  it("keeps the window open when the answer is no", async () => {
    const closing = request();
    await closeGuard(() => true, async () => false)(closing);
    expect(closing.prevented).toBe(true);
  });

  it("closes the window when the answer is yes", async () => {
    const closing = request();
    await closeGuard(() => true, async () => true)(closing);
    expect(closing.prevented).toBe(false);
  });

  it("does not ask a second time while the question is on screen", async () => {
    let answer: (yes: boolean) => void = () => {};
    const ask = vi.fn(() => new Promise<boolean>((resolve) => (answer = resolve)));
    const guard = closeGuard(() => true, ask);
    const first = request();
    const pending = guard(first);
    const second = request();
    await guard(second);
    expect(second.prevented).toBe(true);
    answer(true);
    await pending;
    expect(first.prevented).toBe(false);
    expect(ask).toHaveBeenCalledTimes(1);
  });
});
