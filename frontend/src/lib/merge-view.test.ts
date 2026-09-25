import { describe, expect, it } from "vitest";
import {
  autoResolvedCount,
  chooseAll,
  canSave,
  conflictRows,
  editableText,
  mergeKey,
  mergeRows,
  mergedText,
  nextConflict,
  syntacticCount,
  unresolvedCount,
  unsavedResolution,
} from "./merge-view";
import type { Region } from "$lib/ipc";

const clean = (lines: string[], origin: Region extends never ? never : string = "unchanged") =>
  ({ kind: "clean", lines, origin }) as unknown as Region;

const conflict = (base: string[], ours: string[], theirs: string[]) =>
  ({ kind: "conflict", base, ours, theirs }) as unknown as Region;

const regions: Region[] = [
  clean(["a"]),
  conflict(["b"], ["OURS"], ["THEIRS"]),
  clean(["c"], "ours"),
];

describe("mergeRows", () => {
  it("gives one row per line of a clean region", () => {
    const rows = mergeRows([clean(["a", "b"])], {});
    expect(rows).toHaveLength(2);
    expect(rows.map((row) => row.result)).toEqual(["a", "b"]);
  });

  it("shows the same line in all four columns when nobody changed it", () => {
    const [row] = mergeRows([clean(["a"])], {});
    expect([row?.base, row?.ours, row?.theirs, row?.result]).toEqual(["a", "a", "a", "a"]);
  });

  it("leaves the result empty for an undecided conflict", () => {
    const rows = mergeRows([conflict(["b"], ["OURS"], ["THEIRS"])], {});
    expect(rows[0]?.result).toBeNull();
    expect(rows[0]?.conflict).toBe(true);
  });

  it("fills the result once a side is chosen", () => {
    const rows = mergeRows([conflict(["b"], ["OURS"], ["THEIRS"])], { 0: "ours" });
    expect(rows[0]?.result).toBe("OURS");
  });

  it("pads the short columns so the four panels stay in step", () => {
    const rows = mergeRows([conflict(["b"], ["one", "two"], ["just one"])], {});
    expect(rows).toHaveLength(2);
    expect(rows[1]?.theirs).toBeNull();
    expect(rows[1]?.ours).toBe("two");
  });

  it("marks which side a clean region came from", () => {
    const rows = mergeRows([clean(["c"], "theirs")], {});
    expect(rows[0]?.origin).toBe("theirs");
  });

  it("numbers rows by their region so a click knows what it hit", () => {
    const rows = mergeRows(regions, {});
    expect(rows.map((row) => row.region)).toEqual([0, 1, 2]);
  });

  it("keeps a region that resolved to nothing visible as one empty row", () => {
    // A side deleted the lines; without a row the deletion is invisible in the panels.
    const rows = mergeRows([conflict(["b"], [], ["THEIRS"])], { 0: "ours" });
    expect(rows).toHaveLength(1);
    expect(rows[0]?.ours).toBeNull();
  });
});

describe("conflictRows", () => {
  it("is the row each conflict starts on, in order", () => {
    expect(conflictRows(mergeRows(regions, {}))).toEqual([1]);
  });

  it("is empty when nothing conflicts", () => {
    expect(conflictRows(mergeRows([clean(["a"])], {}))).toEqual([]);
  });
});

describe("unresolvedCount", () => {
  it("counts conflicts nobody has decided", () => {
    expect(unresolvedCount(regions, {})).toBe(1);
  });

  it("drops to zero once every conflict has a side", () => {
    expect(unresolvedCount(regions, { 1: "theirs" })).toBe(0);
  });

  it("ignores a choice recorded against a clean region", () => {
    expect(unresolvedCount(regions, { 0: "ours" })).toBe(1);
  });
});

describe("chooseAll", () => {
  it("picks the same side for every conflict", () => {
    expect(chooseAll(regions, "theirs")).toEqual({ 1: "theirs" });
  });

  it("does not touch clean regions", () => {
    expect(Object.keys(chooseAll(regions, "ours"))).toEqual(["1"]);
  });
});

describe("mergedText", () => {
  it("joins the resolved lines", () => {
    expect(mergedText(regions, { 1: "ours" })).toBe("a\nOURS\nc\n");
  });

  it("takes theirs when theirs is chosen", () => {
    expect(mergedText(regions, { 1: "theirs" })).toBe("a\nTHEIRS\nc\n");
  });

  it("keeps both sides in order when both are wanted", () => {
    expect(mergedText(regions, { 1: "both" })).toBe("a\nOURS\nTHEIRS\nc\n");
  });

  it("falls back to the base for a conflict left undecided", () => {
    // The caller refuses to save while anything is unresolved; this only has to be honest.
    expect(mergedText(regions, {})).toBe("a\nb\nc\n");
  });

  it("ends with a newline, which is what a text file has", () => {
    expect(mergedText([clean(["only"])], {}).endsWith("\n")).toBe(true);
  });

  it("gives an empty file nothing rather than a lone newline", () => {
    expect(mergedText([clean([])], {})).toBe("");
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

describe("editableText", () => {
  it("writes an undecided conflict out with the markers git uses", () => {
    expect(editableText(regions, {})).toBe(
      "a\n<<<<<<< ours\nOURS\n||||||| base\nb\n=======\nTHEIRS\n>>>>>>> theirs\nc\n",
    );
  });

  it("is the merged text once every conflict has a side", () => {
    expect(editableText(regions, { 1: "theirs" })).toBe(mergedText(regions, { 1: "theirs" }));
  });
});

describe("canSave", () => {
  it("refuses the panels while a conflict is undecided", () => {
    expect(canSave(regions, {}, null)).toBe(false);
    expect(canSave(regions, { 1: "ours" }, null)).toBe(true);
  });

  it("refuses hand-edited text that still carries conflict markers", () => {
    expect(canSave(regions, {}, editableText(regions, {}))).toBe(false);
  });

  it("takes hand-edited text once the markers are gone", () => {
    expect(canSave(regions, {}, "a\nmine\nc\n")).toBe(true);
  });

  it("does not mistake a line of equals signs inside other text for a marker", () => {
    expect(canSave(regions, {}, "title\n======= and more\n")).toBe(true);
  });
});

// The merge window closed on Esc, Ctrl+W or its ✕ at once, and the sides picked and the
// text typed into the result were gone without a word (04 §7, 11 §9).
describe("unsavedResolution", () => {
  it("is nothing while no side is picked and nothing typed", () => {
    expect(unsavedResolution({}, null)).toBe(false);
  });

  it("is a side picked for a conflict", () => {
    expect(unsavedResolution({ 1: "ours" }, null)).toBe(true);
  });

  it("is text typed into the result", () => {
    expect(unsavedResolution({}, "resolved by hand")).toBe(true);
  });
});

// 11 §9 promised F6 and Ctrl+1…3 in the merge window; only Ctrl+S did anything.
describe("mergeKey", () => {
  const press = (key: string, code: string, mods: { ctrl?: boolean; shift?: boolean; alt?: boolean } = {}) => ({
    key,
    code,
    ctrl: mods.ctrl ?? false,
    shift: mods.shift ?? false,
    alt: mods.alt ?? false,
  });

  it("walks the conflicts with F6 and Shift+F6", () => {
    expect(mergeKey(press("F6", "F6"))).toEqual({ step: 1 });
    expect(mergeKey(press("F6", "F6", { shift: true }))).toEqual({ step: -1 });
  });

  it("takes a side for the current conflict with Ctrl+1, Ctrl+2 and Ctrl+3, in column order", () => {
    expect(mergeKey(press("1", "Digit1", { ctrl: true }))).toEqual({ take: "theirs", all: false });
    expect(mergeKey(press("2", "Digit2", { ctrl: true }))).toEqual({ take: "both", all: false });
    expect(mergeKey(press("3", "Digit3", { ctrl: true }))).toEqual({ take: "ours", all: false });
  });

  it("takes it for every conflict with Shift, whatever the layout types there", () => {
    expect(mergeKey(press("!", "Digit1", { ctrl: true, shift: true }))).toEqual({ take: "theirs", all: true });
    expect(mergeKey(press("#", "Digit3", { ctrl: true, shift: true }))).toEqual({ take: "ours", all: true });
  });

  it("saves with Ctrl+S on any layout", () => {
    expect(mergeKey(press("s", "KeyS", { ctrl: true }))).toBe("save");
    expect(mergeKey(press("ы", "KeyS", { ctrl: true }))).toBe("save");
  });

  it("leaves every other key alone", () => {
    expect(mergeKey(press("1", "Digit1"))).toBeNull();
    expect(mergeKey(press("4", "Digit4", { ctrl: true }))).toBeNull();
    expect(mergeKey(press("1", "Digit1", { ctrl: true, alt: true }))).toBeNull();
    expect(mergeKey(press("a", "KeyA", { ctrl: true }))).toBeNull();
  });
});
