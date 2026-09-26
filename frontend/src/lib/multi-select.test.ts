import { describe, expect, it } from "vitest";
import {
  afterDeselect,
  applyClick,
  EMPTY_SELECTION,
  markedRows,
  rowKey,
  rowOf,
  shownMarks,
  type FileSelection,
} from "./multi-select";

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

describe("rows of a list in sections", () => {
  // b is part staged: [a, b, c | b, x]. By path, Shift from x to the b in Staged took c
  // from Unstaged, since the range was measured from the first b.
  const unstaged = ["a", "b", "c"].map((path) => rowKey(0, path));
  const staged = ["b", "x"].map((path) => rowKey(1, path));
  const shift = { ctrl: false, shift: true };

  it("keeps a Shift range in its own section", () => {
    const marks = applyClick(select([rowKey(1, "x")], rowKey(1, "x")), rowKey(1, "b"), staged, shift);
    expect([...marks.paths].sort()).toEqual([rowKey(1, "b"), rowKey(1, "x")]);
  });

  it("ticks the b of one section without the b of the other", () => {
    const marks = applyClick(EMPTY_SELECTION, rowKey(1, "b"), staged, { ctrl: true, shift: false });
    const rows = markedRows(marks.paths);
    expect([...(rows.bySection.get(1) ?? [])]).toEqual(["b"]);
    expect(rows.bySection.get(0)).toBeUndefined();
  });

  it("does not stretch a Shift range from the other section", () => {
    const marks = applyClick(select([rowKey(0, "a")], rowKey(0, "a")), rowKey(1, "x"), staged, shift);
    expect([...marks.paths]).toEqual([rowKey(1, "x")]);
    expect(unstaged).not.toContain(marks.anchor);
  });

  it("names a path ticked in both sections once", () => {
    expect(markedRows([rowKey(0, "b"), rowKey(1, "b"), rowKey(1, "x")]).paths).toEqual(["b", "x"]);
  });

  it("reads back a path with a colon in it", () => {
    expect(rowOf(rowKey(2, "a:b/c.txt"))).toEqual({ section: 2, path: "a:b/c.txt" });
  });
});

describe("shownMarks", () => {
  it("leaves out the marked files the list no longer shows", () => {
    const marked = select(["a.txt", "gone.txt", "c.txt"], "gone.txt");
    const shown = shownMarks(marked, ORDER);
    expect([...shown.paths]).toEqual(["a.txt", "c.txt"]);
    expect(shown.anchor).toBeNull();
  });

  it("keeps a selection that is all on screen as it is", () => {
    const marked = select(["a.txt", "b.txt"], "b.txt");
    expect(shownMarks(marked, ORDER)).toBe(marked);
  });
});
