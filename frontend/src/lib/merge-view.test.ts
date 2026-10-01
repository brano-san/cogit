import { describe, expect, it } from "vitest";
import {
  autoResolvedCount,
  conflictCount,
  conflictRows,
  nextConflict,
  panelConflictStep,
  previewRows,
  syntacticCount,
} from "./merge-view";
import type { Region } from "$lib/ipc";

// In the Diff panel of the main window F6 always walked the panels past a conflict on
// screen: the merge there took no keys, where a diff steps through its changes (DF-011).
describe("panelConflictStep", () => {
  const f6 = { key: "F6", ctrl: false, alt: false, shift: false };
  const back = { ...f6, shift: true };

  it("steps to the next or the previous conflict while there is one that way", () => {
    expect(panelConflictStep(f6, [3, 9], null)).toBe(1);
    expect(panelConflictStep(f6, [3, 9], 3)).toBe(1);
    expect(panelConflictStep(back, [3, 9], 9)).toBe(-1);
  });

  it("leaves F6 to the panel walk past the last conflict and before the first", () => {
    expect(panelConflictStep(f6, [3, 9], 9)).toBeNull();
    expect(panelConflictStep(back, [3, 9], 3)).toBeNull();
    expect(panelConflictStep(back, [3, 9], null)).toBeNull();
    expect(panelConflictStep({ ...f6, ctrl: true }, [3, 9], null)).toBeNull();
  });
});

const clean = (lines: string[], origin: Region extends never ? never : string = "unchanged") =>
  ({ kind: "clean", lines, origin }) as unknown as Region;

const conflict = (base: string[], ours: string[], theirs: string[]) =>
  ({ kind: "conflict", base, ours, theirs }) as unknown as Region;

const regions: Region[] = [
  clean(["a"]),
  conflict(["b"], ["OURS"], ["THEIRS"]),
  clean(["c"], "ours"),
];

describe("previewRows", () => {
  it("gives one row per line of a clean region, the same line in all three panels", () => {
    const rows = previewRows([clean(["a", "b"])]);
    expect(rows).toHaveLength(2);
    expect(rows.map((row) => [row.ours, row.result, row.theirs])).toEqual([
      ["a", "a", "a"],
      ["b", "b", "b"],
    ]);
  });

  it("shows a conflict's two sides and, as its Result, the lines it could not settle", () => {
    const [row] = previewRows([conflict(["base"], ["OURS"], ["THEIRS"])]);
    expect(row).toMatchObject({ conflict: true, ours: "OURS", result: "base", theirs: "THEIRS" });
  });

  it("pads the short columns so the three panels stay in step", () => {
    const rows = previewRows([conflict(["b"], ["o1", "o2", "o3"], ["t1"])]);
    expect(rows).toHaveLength(3);
    expect(rows.map((row) => row.theirs)).toEqual(["t1", null, null]);
    expect(rows.map((row) => row.result)).toEqual(["b", null, null]);
  });

  it("marks which side a clean region came from", () => {
    const rows = previewRows(regions);
    expect(rows.map((row) => row.origin)).toEqual(["unchanged", null, "ours"]);
  });

  it("numbers rows by their region so a click knows what it hit", () => {
    expect(previewRows(regions).map((row) => row.region)).toEqual([0, 1, 2]);
  });

  it("keeps a region that resolved to nothing visible as one empty row", () => {
    const rows = previewRows([clean([])]);
    expect(rows).toHaveLength(1);
    expect(rows[0]?.result).toBeNull();
  });
});

describe("conflictRows", () => {
  it("is the row each conflict starts on, in order", () => {
    expect(conflictRows(previewRows([clean(["a"]), conflict(["b"], ["B1", "B2"], ["b"]), clean(["c"]), conflict(["d"], ["D"], ["d2"])]))).toEqual([1, 4]);
  });

  it("is empty when nothing conflicts", () => {
    expect(conflictRows(previewRows([clean(["a"])]))).toEqual([]);
  });
});

describe("conflictCount", () => {
  it("counts the conflicts of the merge", () => {
    expect(conflictCount(regions)).toBe(1);
    expect(conflictCount([clean(["a"])])).toBe(0);
  });
});

describe("autoResolvedCount", () => {
  it("counts the regions taken without asking", () => {
    expect(autoResolvedCount(regions)).toBe(1);
  });

  it("does not count lines nobody touched", () => {
    expect(autoResolvedCount([clean(["a"])])).toBe(0);
  });

  it("counts a region both sides changed the same way", () => {
    expect(autoResolvedCount([clean(["a"], "both")])).toBe(1);
  });
});

describe("nextConflict", () => {
  const at = [3, 9, 14];

  it("steps to the one below", () => {
    expect(nextConflict(at, 3, 1)).toBe(9);
  });

  it("steps to the one above", () => {
    expect(nextConflict(at, 9, -1)).toBe(3);
  });

  it("wraps at the end so the last one is not a dead end", () => {
    expect(nextConflict(at, 14, 1)).toBe(3);
  });

  it("wraps at the start too", () => {
    expect(nextConflict(at, 3, -1)).toBe(14);
  });

  it("starts at the first when nothing is current", () => {
    expect(nextConflict(at, null, 1)).toBe(3);
  });

  it("goes to the next one below a row between conflicts", () => {
    expect(nextConflict(at, 5, 1)).toBe(9);
  });

  it("has nowhere to go in a file without conflicts", () => {
    expect(nextConflict([], null, 1)).toBeNull();
  });
});

describe("syntacticCount", () => {
  it("counts only what the parser settled", () => {
    expect(syntacticCount([clean(["a"], "syntactic"), clean(["b"], "ours")])).toBe(1);
  });

  it("is zero when no parser was involved", () => {
    expect(syntacticCount([clean(["a"], "ours")])).toBe(0);
  });
});

describe("autoResolvedCount with a parser", () => {
  it("counts a parser-settled region too: it still wants a look", () => {
    expect(autoResolvedCount([clean(["a"], "syntactic")])).toBe(1);
  });
});

