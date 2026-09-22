import { describe, expect, it } from "vitest";
import { closesWindow } from "./child-window";

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
