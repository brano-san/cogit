import { describe, expect, it } from "vitest";
import { diffKey, type DiffKeyState, type DiffPress } from "./diff-keys";

const press = (over: Partial<DiffPress>): DiffPress => ({ key: "", ctrl: false, shift: false, alt: false, ...over });
const state = (over: Partial<DiffKeyState> = {}): DiffKeyState => ({
  active: true,
  findShowing: false,
  prev: true,
  next: true,
  ...over,
});

describe("diffKey", () => {
  it("steps through the changes with F6 while the diff has the keyboard", () => {
    expect(diffKey(press({ key: "F6" }), state())).toEqual({ kind: "jump", by: 1 });
    expect(diffKey(press({ key: "F6", shift: true }), state())).toEqual({ kind: "jump", by: -1 });
  });

  // F6 in Graph moved the focus to the next panel and scrolled the diff at the same time.
  it("leaves F6 to the panel walk while another panel has the focus", () => {
    expect(diffKey(press({ key: "F6" }), state({ active: false }))).toBeNull();
  });

  it("hands F6 back to the panel walk past the last change, so the keyboard can leave", () => {
    expect(diffKey(press({ key: "F6" }), state({ next: false }))).toBeNull();
    expect(diffKey(press({ key: "F6", shift: true }), state({ prev: false }))).toBeNull();
    expect(diffKey(press({ key: "F6", shift: true }), state({ next: false }))).toEqual({ kind: "jump", by: -1 });
  });

  // Ctrl+F in the commit message box pulled the focus into the diff's search.
  it("keeps its search, Investigate and Esc to itself while it has the keyboard", () => {
    const find = press({ key: "f", code: "KeyF", ctrl: true });
    expect(diffKey(find, state())).toEqual({ kind: "find" });
    expect(diffKey(find, state({ active: false }))).toBeNull();
    const investigate = press({ key: "L", code: "KeyL", ctrl: true, alt: true, shift: true });
    expect(diffKey(investigate, state())).toEqual({ kind: "investigate" });
    expect(diffKey(investigate, state({ active: false }))).toBeNull();
    expect(diffKey(press({ key: "Escape" }), state({ findShowing: true }))).toEqual({ kind: "close-find" });
    expect(diffKey(press({ key: "Escape" }), state({ findShowing: true, active: false }))).toBeNull();
    expect(diffKey(press({ key: "Escape" }), state())).toBeNull();
  });

  it("switches the layout from anywhere, as 11 §7 has it", () => {
    const toggle = press({ key: "D", code: "KeyD", ctrl: true, shift: true });
    expect(diffKey(toggle, state())).toEqual({ kind: "layout" });
    expect(diffKey(toggle, state({ active: false }))).toEqual({ kind: "layout" });
  });

  it("reads letters by where they sit, with any layout", () => {
    expect(diffKey(press({ key: "а", code: "KeyF", ctrl: true }), state())).toEqual({ kind: "find" });
  });

  it("does not take a chord it has no use for", () => {
    expect(diffKey(press({ key: "F6", ctrl: true }), state())).toBeNull();
    expect(diffKey(press({ key: "f", code: "KeyF", ctrl: true, alt: true }), state())).toBeNull();
  });
});
