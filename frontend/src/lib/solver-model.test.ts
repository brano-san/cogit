import { describe, expect, it } from "vitest";
import type { SolverRegion } from "./ipc";
import {
  buildResult,
  composeSave,
  conflictsLeftLabel,
  docsForDeleted,
  docsFromRegions,
  equalLines,
  hunkTexts,
  isUnresolved,
  linesToText,
  nextHunk,
  paneTone,
  sideRows,
  takeLines,
  textToLines,
  visibleActions,
  type Hunk,
} from "./solver-model";

function region(kind: SolverRegion["kind"], parts: Partial<Omit<SolverRegion, "kind">> = {}): SolverRegion {
  return { kind, base: [], ours: [], theirs: [], result: [], ...parts };
}

const REGIONS: SolverRegion[] = [
  region("equal", { base: ["a"], ours: ["a"], theirs: ["a"], result: ["a"] }),
  region("conflict", { base: ["b"], ours: ["B"], theirs: ["b2", "b3"], result: ["b"] }),
  region("equal", { base: ["c"], ours: ["c"], theirs: ["c"], result: ["c"] }),
  region("ours", { base: ["d"], ours: ["D"], theirs: ["d"], result: ["D"] }),
];

describe("lines and text", () => {
  it("every line ends with a newline, and nothing is the empty string", () => {
    expect(linesToText(["a", "b"])).toBe("a\nb\n");
    expect(linesToText([])).toBe("");
    expect(linesToText([""])).toBe("\n");
  });

  it("text reads back to the same lines", () => {
    expect(textToLines("a\nb\n")).toEqual(["a", "b"]);
    expect(textToLines("")).toEqual([]);
    expect(textToLines("a\n\n")).toEqual(["a", ""]);
    expect(textToLines("no newline")).toEqual(["no newline"]);
  });

  it("compares lines by value", () => {
    expect(equalLines(["a"], ["a"])).toBe(true);
    expect(equalLines(["a"], ["a", "b"])).toBe(false);
    expect(equalLines([], [])).toBe(true);
  });
});

describe("buildResult", () => {
  it("lays the regions out as one document and records where each stretch that is not equal sits", () => {
    const built = buildResult(REGIONS);
    expect(built.text).toBe("a\nb\nc\nD\n");
    expect(built.hunks.map((hunk) => [hunk.id, hunk.kind])).toEqual([
      [1, "conflict"],
      [3, "ours"],
    ]);
    expect(built.spans).toEqual([
      { id: 1, start: 1, count: 1 },
      { id: 3, start: 3, count: 1 },
    ]);
  });

  it("a conflict starts on the base lines, which is how it reads as undecided", () => {
    const built = buildResult(REGIONS);
    expect(built.hunks[0]?.initial).toEqual(["b"]);
  });

  it("a stretch that merged to nothing is a block of no lines", () => {
    const built = buildResult([
      region("equal", { base: ["a"], ours: ["a"], theirs: ["a"], result: ["a"] }),
      region("theirs", { base: ["x"], ours: ["x"], theirs: [], result: [] }),
    ]);
    expect(built.text).toBe("a\n");
    expect(built.spans).toEqual([{ id: 1, start: 1, count: 0 }]);
  });
});

describe("take actions", () => {
  const conflict = buildResult(REGIONS).hunks[0] as Hunk;

  it("puts the lines of one side, or of both in either order", () => {
    expect(takeLines(conflict, "ours")).toEqual(["B"]);
    expect(takeLines(conflict, "theirs")).toEqual(["b2", "b3"]);
    expect(takeLines(conflict, "oursTheirs")).toEqual(["B", "b2", "b3"]);
    expect(takeLines(conflict, "theirsOurs")).toEqual(["b2", "b3", "B"]);
  });

  it("returns copies, so editing the result never changes a side", () => {
    expect(takeLines(conflict, "ours")).not.toBe(conflict.ours);
  });
});

describe("what is unresolved", () => {
  const [conflict, auto] = buildResult(REGIONS).hunks as [Hunk, Hunk];

  it("a conflict is unresolved while the result holds its base lines", () => {
    expect(isUnresolved(conflict, ["b"])).toBe(true);
    expect(isUnresolved(conflict, ["B"])).toBe(false);
    expect(isUnresolved(conflict, ["b", "more"])).toBe(false);
  });

  it("a conflict that is edited back to the base is unresolved again, which is what Undo needs", () => {
    expect(isUnresolved(conflict, ["B"])).toBe(false);
    expect(isUnresolved(conflict, ["b"])).toBe(true);
  });

  it("a stretch the merge settled is never a conflict", () => {
    expect(isUnresolved(auto, ["whatever"])).toBe(false);
  });

  it("the counter says what is left", () => {
    expect(conflictsLeftLabel(0)).toBe("No conflicts left");
    expect(conflictsLeftLabel(1)).toBe("1 conflict left");
    expect(conflictsLeftLabel(4)).toBe("4 conflicts left");
  });
});

describe("saving with markers", () => {
  const { hunks, spans } = buildResult(REGIONS);
  const labels = { ours: "main", theirs: "feature" };

  it("an unresolved conflict is written as git writes it", () => {
    const text = composeSave(["a", "b", "c", "D"], spans, hunks, labels);
    expect(text).toBe(
      ["a", "<<<<<<< main", "B", "=======", "b2", "b3", ">>>>>>> feature", "c", "D", ""].join("\n"),
    );
  });

  it("a resolved conflict is written as it stands", () => {
    expect(composeSave(["a", "B", "c", "D"], spans, hunks, labels)).toBe("a\nB\nc\nD\n");
  });

  it("an edit anywhere else is kept", () => {
    expect(composeSave(["a!", "B", "c", "D", "extra"], spans, hunks, labels)).toBe("a!\nB\nc\nD\nextra\n");
  });

  it("spans after a longer block are shifted by the markers already written", () => {
    const both = [
      region("conflict", { base: ["1"], ours: ["o1"], theirs: ["t1"], result: ["1"] }),
      region("equal", { base: ["m"], ours: ["m"], theirs: ["m"], result: ["m"] }),
      region("conflict", { base: ["2"], ours: ["o2"], theirs: ["t2"], result: ["2"] }),
    ];
    const built = buildResult(both);
    const text = composeSave(["1", "m", "2"], built.spans, built.hunks, labels);
    expect(text.split("\n").filter((line) => line.startsWith("<<<<<<<")).length).toBe(2);
    expect(text).toContain("m\n<<<<<<< main\no2\n=======\nt2\n>>>>>>> feature\n");
  });
});

describe("what the buttons in a gutter offer", () => {
  const [conflict, auto] = buildResult(REGIONS).hunks as [Hunk, Hunk];

  it("there is no point in taking what the result already holds", () => {
    expect(visibleActions(conflict, ["b"])).toEqual({ ours: true, theirs: true });
    expect(visibleActions(conflict, ["B"])).toEqual({ ours: false, theirs: true });
    expect(visibleActions(conflict, ["b2", "b3"])).toEqual({ ours: true, theirs: false });
    expect(visibleActions(auto, ["D"])).toEqual({ ours: false, theirs: true });
  });
});

describe("walking through hunks", () => {
  it("steps in document order and wraps around", () => {
    const ids = [1, 3, 7];
    expect(nextHunk(ids, null, 1)).toBe(1);
    expect(nextHunk(ids, null, -1)).toBe(7);
    expect(nextHunk(ids, 1, 1)).toBe(3);
    expect(nextHunk(ids, 7, 1)).toBe(1);
    expect(nextHunk(ids, 3, -1)).toBe(1);
    expect(nextHunk(ids, 1, -1)).toBe(7);
  });

  it("from a hunk that is no longer in the list, goes to the neighbour on that side", () => {
    expect(nextHunk([1, 7], 3, 1)).toBe(7);
    expect(nextHunk([1, 7], 3, -1)).toBe(1);
  });

  it("with nothing to walk to, stays nowhere", () => {
    expect(nextHunk([], 3, 1)).toBeNull();
  });
});

describe("how a pane colors a hunk", () => {
  const conflict = buildResult(REGIONS).hunks[0] as Hunk;
  const added: Hunk = { id: 5, kind: "theirs", base: [], ours: [], theirs: ["new"], initial: ["new"] };
  const removed: Hunk = { id: 6, kind: "ours", base: ["gone"], ours: [], theirs: ["gone"], initial: [] };
  const bothSame: Hunk = { id: 7, kind: "both", base: ["x"], ours: ["y"], theirs: ["y"], initial: ["y"] };

  it("without Base Changes, a hunk is colored where the sides differ from each other", () => {
    expect(paneTone(conflict, "ours", false)).toBe("changed");
    expect(paneTone(conflict, "theirs", false)).toBe("changed");
    expect(paneTone(bothSame, "ours", false)).toBeNull();
  });

  it("with Base Changes, a side is colored by what it did to the base", () => {
    expect(paneTone(added, "theirs", true)).toBe("add");
    expect(paneTone(added, "ours", true)).toBeNull();
    expect(paneTone(removed, "ours", true)).toBe("del");
    expect(paneTone(removed, "theirs", true)).toBeNull();
    expect(paneTone(conflict, "ours", true)).toBe("changed");
    expect(paneTone(bothSame, "ours", true)).toBe("changed");
  });
});

describe("hunkTexts", () => {
  it("reads a hunk's current lines out of the document by its span", () => {
    const lines = ["a", "B", "c"];
    expect(hunkTexts(lines, [{ id: 1, start: 1, count: 1 }])).toEqual(new Map([[1, ["B"]]]));
    expect(hunkTexts(lines, [{ id: 9, start: 3, count: 0 }])).toEqual(new Map([[9, []]]));
  });
});

describe("sideRows", () => {
  it("is where each hunk sits in Ours and in Theirs, which are never edited", () => {
    expect(sideRows(REGIONS, "ours")).toEqual([
      { start: 1, count: 1 },
      { start: 3, count: 1 },
    ]);
    expect(sideRows(REGIONS, "theirs")).toEqual([
      { start: 1, count: 2 },
      { start: 4, count: 1 },
    ]);
  });
});

describe("the documents of the three panes", () => {
  it("are the sides and the merge laid out line by line, with every hunk's place in each", () => {
    const docs = docsFromRegions(REGIONS);
    expect(docs.oursText).toBe("a\nB\nc\nD\n");
    expect(docs.theirsText).toBe("a\nb2\nb3\nc\nd\n");
    expect(docs.resultText).toBe("a\nb\nc\nD\n");
    expect(docs.hunks.map((hunk) => hunk.id)).toEqual([1, 3]);
    expect(docs.oursRows).toEqual([
      { start: 1, count: 1 },
      { start: 3, count: 1 },
    ]);
    expect(docs.theirsRows).toEqual([
      { start: 1, count: 2 },
      { start: 4, count: 1 },
    ]);
    expect(docs.spans).toEqual([
      { id: 1, start: 1, count: 1 },
      { id: 3, start: 3, count: 1 },
    ]);
  });

  it("for a file one side deleted, the other side is the Result to start from and there are no hunks", () => {
    const docs = docsForDeleted({ ours: null, theirs: "keep\nme\n" });
    expect(docs.oursText).toBe("");
    expect(docs.theirsText).toBe("keep\nme\n");
    expect(docs.resultText).toBe("keep\nme\n");
    expect(docs.hunks).toEqual([]);
    expect(docsForDeleted({ ours: "mine\n", theirs: null }).resultText).toBe("mine\n");
    expect(docsForDeleted({ ours: null, theirs: null }).resultText).toBe("");
  });
});
