import { describe, expect, it } from "vitest";
import { closesWindow, whenCloses } from "./child-window";

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
