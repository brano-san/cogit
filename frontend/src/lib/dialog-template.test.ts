import { describe, expect, it } from "vitest";
import { initialFocus, isClamped, parseStashMessage } from "./dialog-template";

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
