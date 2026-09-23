import { describe, expect, it } from "vitest";
import { afterDeselect, applyClick, EMPTY_SELECTION, type FileSelection } from "./multi-select";

const ORDER = ["a.txt", "b.txt", "c.txt", "d.txt"];
const plain = { ctrl: false, shift: false };

function select(paths: string[], anchor: string | null = null): FileSelection {
  return { paths: new Set(paths), anchor };
}

describe("applyClick", () => {
  it("a plain click selects exactly one file", () => {
    const next = applyClick(select(["a.txt", "b.txt"]), "c.txt", ORDER, plain);
    expect([...next.paths]).toEqual(["c.txt"]);
  });

  it("a plain click makes that file the anchor", () => {
    expect(applyClick(EMPTY_SELECTION, "b.txt", ORDER, plain).anchor).toBe("b.txt");
  });

  it("ctrl-click adds to the selection", () => {
    const next = applyClick(select(["a.txt"]), "c.txt", ORDER, { ctrl: true, shift: false });
    expect([...next.paths].sort()).toEqual(["a.txt", "c.txt"]);
  });

  it("ctrl-click removes a file that was already selected", () => {
    const next = applyClick(select(["a.txt", "c.txt"]), "c.txt", ORDER, {
      ctrl: true,
      shift: false,
    });
    expect([...next.paths]).toEqual(["a.txt"]);
  });

  it("shift-click selects the range from the anchor", () => {
    const next = applyClick(select(["a.txt"], "a.txt"), "c.txt", ORDER, {
      ctrl: false,
      shift: true,
    });
    expect([...next.paths].sort()).toEqual(["a.txt", "b.txt", "c.txt"]);
  });

  it("shift-click works backwards too", () => {
    const next = applyClick(select(["d.txt"], "d.txt"), "b.txt", ORDER, {
      ctrl: false,
      shift: true,
    });
    expect([...next.paths].sort()).toEqual(["b.txt", "c.txt", "d.txt"]);
  });

  it("shift-click without an anchor behaves like a plain click", () => {
    const next = applyClick(EMPTY_SELECTION, "c.txt", ORDER, { ctrl: false, shift: true });
    expect([...next.paths]).toEqual(["c.txt"]);
  });

  it("shift-click keeps the anchor where it was", () => {
    const next = applyClick(select(["a.txt"], "a.txt"), "c.txt", ORDER, {
      ctrl: false,
      shift: true,
    });
    expect(next.anchor).toBe("a.txt");
  });

  it("a click on a file that is not in the list changes nothing", () => {
    const before = select(["a.txt"], "a.txt");
    expect(applyClick(before, "ghost.txt", ORDER, plain)).toBe(before);
  });

  it("does not mutate the selection it was given", () => {
    const before = select(["a.txt"], "a.txt");
    applyClick(before, "c.txt", ORDER, { ctrl: true, shift: false });
    expect([...before.paths]).toEqual(["a.txt"]);
  });
});

describe("afterDeselect", () => {
  it("drops the marks once the shown file is let go, as a re-clicked commit does (#7)", () => {
    expect(afterDeselect("a.txt", null, select(["a.txt", "b.txt"], "a.txt"))).toBe(EMPTY_SELECTION);
  });

  it("keeps marks made while nothing was shown: ctrl-clicks do not open a file", () => {
    const marked = select(["a.txt", "b.txt"], "b.txt");
    expect(afterDeselect(null, null, marked)).toBe(marked);
  });

  it("keeps the marks when another file is shown", () => {
    const marked = select(["a.txt"], "a.txt");
    expect(afterDeselect("a.txt", "b.txt", marked)).toBe(marked);
  });
});
