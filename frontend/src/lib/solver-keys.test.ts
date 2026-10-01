import { describe, expect, it } from "vitest";
import { solverKey } from "./solver-keys";

const press = (key: string, extra: Partial<{ code: string; ctrl: boolean; shift: boolean; alt: boolean }> = {}) => ({
  key,
  code: extra.code,
  ctrl: extra.ctrl ?? false,
  shift: extra.shift ?? false,
  alt: extra.alt ?? false,
});

describe("the keys of the Conflict Solver", () => {
  it("Ctrl+S saves, on any layout", () => {
    expect(solverKey(press("s", { ctrl: true, code: "KeyS" }))).toBe("save");
    expect(solverKey(press("ы", { ctrl: true, code: "KeyS" }))).toBe("save");
    expect(solverKey(press("s", { ctrl: true, shift: true, code: "KeyS" }))).toBeNull();
  });

  it("F6 steps through the changes and F7 through the conflicts, Shift going back", () => {
    expect(solverKey(press("F6"))).toEqual({ step: "change", by: 1 });
    expect(solverKey(press("F6", { shift: true }))).toEqual({ step: "change", by: -1 });
    expect(solverKey(press("F7"))).toEqual({ step: "conflict", by: 1 });
    expect(solverKey(press("F7", { shift: true }))).toEqual({ step: "conflict", by: -1 });
    expect(solverKey(press("F6", { ctrl: true }))).toBeNull();
  });

  it("Ctrl+1 to Ctrl+4 take a side for the current hunk, by the place of the key", () => {
    expect(solverKey(press("1", { ctrl: true, code: "Digit1" }))).toEqual({ take: "ours" });
    expect(solverKey(press("2", { ctrl: true, code: "Digit2" }))).toEqual({ take: "theirs" });
    expect(solverKey(press("3", { ctrl: true, code: "Digit3" }))).toEqual({ take: "oursTheirs" });
    expect(solverKey(press("4", { ctrl: true, code: "Digit4" }))).toEqual({ take: "theirsOurs" });
    expect(solverKey(press("!", { ctrl: true, code: "Digit1" }))).toEqual({ take: "ours" });
  });

  it("leaves the editor's own keys alone", () => {
    expect(solverKey(press("z", { ctrl: true, code: "KeyZ" }))).toBeNull();
    expect(solverKey(press("a", { ctrl: true, code: "KeyA" }))).toBeNull();
    expect(solverKey(press("1", { code: "Digit1" }))).toBeNull();
    expect(solverKey(press("1", { ctrl: true, shift: true, alt: true, code: "Digit1" }))).toBeNull();
  });
});
