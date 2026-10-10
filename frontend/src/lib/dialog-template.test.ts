import { describe, expect, it } from "vitest";
import { confirmsDialog, initialFocus, isClamped, parseStashMessage } from "./dialog-template";

const act = (primary = false) => ({ label: "x", onclick: () => {}, primary });

describe("initialFocus", () => {
  it("starts on Cancel for a destructive primary", () => {
    expect(initialFocus(true, [act(true)])).toBe("cancel");
  });
  it("starts on the primary action otherwise", () => {
    expect(initialFocus(false, [act(), act(true)])).toBe(1);
  });
  it("falls back to Cancel without a primary", () => {
    expect(initialFocus(false, [act()])).toBe("cancel");
  });
});

describe("isClamped", () => {
  it("tolerates a pixel of rounding", () => {
    expect(isClamped(60, 60)).toBe(false);
    expect(isClamped(61, 60)).toBe(false);
    expect(isClamped(80, 60)).toBe(true);
  });
});

describe("parseStashMessage", () => {
  it("splits the branch off", () => {
    expect(parseStashMessage("WIP on main: abc1234 fix")).toEqual({ branch: "main", message: "abc1234 fix" });
    expect(parseStashMessage("On dev: my work\nline 2")).toEqual({ branch: "dev", message: "my work\nline 2" });
  });
  it("keeps a free-form message whole", () => {
    expect(parseStashMessage("custom")).toEqual({ branch: null, message: "custom" });
  });
});

describe("confirming a dialog from the keyboard", () => {
  const press = (over: Partial<{ key: string; isComposing: boolean; defaultPrevented: boolean; ctrlKey: boolean; metaKey: boolean }> = {}) => ({
    key: "Enter",
    isComposing: false,
    defaultPrevented: false,
    ctrlKey: false,
    metaKey: false,
    ...over,
  });

  it("takes Enter from a field, not from a textarea or a button", () => {
    expect(confirmsDialog(press(), "field", false)).toBe(true);
    expect(confirmsDialog(press(), "text", false)).toBe(false);
    expect(confirmsDialog(press(), "button", false)).toBe(false);
  });

  it("takes Ctrl+Enter, ⌘+Enter on a Mac, from anywhere", () => {
    expect(confirmsDialog(press({ ctrlKey: true }), "text", false)).toBe(true);
    expect(confirmsDialog(press({ ctrlKey: true }), "button", false)).toBe(true);
    expect(confirmsDialog(press({ metaKey: true }), "text", true)).toBe(true);
    expect(confirmsDialog(press({ metaKey: true }), "text", false)).toBe(false);
  });

  it("leaves a key something already used, or one still being composed", () => {
    expect(confirmsDialog(press({ ctrlKey: true, defaultPrevented: true }), "field", false)).toBe(false);
    expect(confirmsDialog(press({ isComposing: true }), "field", false)).toBe(false);
    expect(confirmsDialog(press({ key: "a", ctrlKey: true }), "field", false)).toBe(false);
  });
});
