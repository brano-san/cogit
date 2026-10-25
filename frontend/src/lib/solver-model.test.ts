import { describe, expect, it } from "vitest";
import type { SolverRegion } from "./ipc";
import { foldsOf } from "./solver-geometry";
import {
  hunkLegend,
  hunkTone,
  buildResult,
  conflictsLeftLabel,
  docsFromRegions,
  sideTone,
  solverContext,
  solverDocs,
  takeChanges,
  paneTitle,
  paneTooltip,
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

/** The conflict of REGIONS as the Result shows it before a decision. */
const MARKED = ["<<<<<<< ours", "B", "=======", "b2", "b3", ">>>>>>> theirs"];

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
    expect(built.text).toBe(["a", ...MARKED, "c", "D", ""].join("\n"));
    expect(built.hunks.map((hunk) => [hunk.id, hunk.kind])).toEqual([
      [1, "conflict"],
      [3, "ours"],
    ]);
    expect(built.spans).toEqual([
      { id: 1, start: 1, count: 6 },
      { id: 3, start: 8, count: 1 },
    ]);
  });

  it("a conflict starts as git writes it, both sides between markers named by the labels", () => {
    const built = buildResult(REGIONS, { ours: "HEAD", theirs: "feature" });
    expect(built.hunks[0]?.initial).toEqual(["<<<<<<< HEAD", "B", "=======", "b2", "b3", ">>>>>>> feature"]);
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

  it("can put the base back, which neither side holds", () => {
    expect(takeLines(conflict, "base")).toEqual(["b"]);
    expect(takeLines(conflict, "base")).not.toBe(conflict.base);
  });
});

describe("what is unresolved", () => {
  const [conflict, auto] = buildResult(REGIONS).hunks as [Hunk, Hunk];

  it("a conflict is unresolved while a marker is left in its stretch", () => {
    expect(isUnresolved(conflict, MARKED)).toBe(true);
    expect(isUnresolved(conflict, ["B", "=======", "b2"])).toBe(true);
    expect(isUnresolved(conflict, ["B"])).toBe(false);
    expect(isUnresolved(conflict, ["b"])).toBe(false);
  });

  it("a Take decides it, whatever is edited in later", () => {
    expect(isUnresolved(conflict, MARKED, true)).toBe(false);
  });

  it("a stretch the merge settled is never a conflict", () => {
    expect(isUnresolved(auto, ["<<<<<<< x"])).toBe(false);
  });

  it("the counter says what is left", () => {
    expect(conflictsLeftLabel(0)).toBe("No conflicts left");
    expect(conflictsLeftLabel(1)).toBe("1 conflict left");
    expect(conflictsLeftLabel(4)).toBe("4 conflicts left");
  });
});

describe("what the buttons in a gutter offer", () => {
  const [conflict, auto] = buildResult(REGIONS).hunks as [Hunk, Hunk];

  it("there is no point in taking what the result already holds", () => {
    expect(visibleActions(conflict, MARKED)).toEqual({ ours: true, theirs: true });
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
    expect(docs.resultText).toBe(["a", ...MARKED, "c", "D", ""].join("\n"));
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
      { id: 1, start: 1, count: 6 },
      { id: 3, start: 8, count: 1 },
    ]);
  });

});

/** Folds of all three panes at 3 context lines; a small file is not folded at all. */
function foldsAt(docs: ReturnType<typeof solverDocs>, context = 3) {
  const lines = (text: string) => Math.max(textToLines(text).length, 1);
  const at = solverContext(context, docs);
  const resultRows = docs.spans.map((span) => ({ start: span.start, count: span.count }));
  return [
    foldsOf(docs.oursRows, lines(docs.oursText), at, new Set()),
    foldsOf(resultRows, lines(docs.resultText), at, new Set()),
    foldsOf(docs.theirsRows, lines(docs.theirsText), at, new Set()),
  ];
}

const many = (prefix: string, count: number) => Array.from({ length: count }, (_, at) => `${prefix}${at}`);

describe("solverDocs", () => {
  it("modify/delete: Ours is empty, the Result starts as Theirs, all of it one block added to Ours", () => {
    const docs = solverDocs({ base: "step 4\n", ours: null, theirs: "step 4\nstep 4 extra\n", regions: [] });
    expect(docs.oursText).toBe("");
    expect(docs.resultText).toBe("step 4\nstep 4 extra\n");
    expect(docs.hunks).toHaveLength(1);
    expect(docs.oursRows).toEqual([{ start: 0, count: 0 }]);
    expect(docs.theirsRows).toEqual([{ start: 0, count: 2 }]);
    expect(docs.spans).toEqual([{ id: 0, start: 0, count: 2 }]);
    const hunk = docs.hunks[0]!;
    const result = ["step 4", "step 4 extra"];
    expect(sideTone(hunk.ours, result)).toBe("add");
    expect(sideTone(hunk.theirs, result)).toBeNull();
    expect(hunkLegend(hunk, false)).toMatch(/Ours deleted/);
    expect(foldsAt(docs)).toEqual([[], [], []]);
  });

  it("delete/modify: the mirror image, Theirs empty and the Result as Ours has it", () => {
    const docs = solverDocs({ base: "a\n", ours: "a\nb\n", theirs: null, regions: [] });
    expect(docs.theirsText).toBe("");
    expect(docs.resultText).toBe("a\nb\n");
    expect(docs.theirsRows).toEqual([{ start: 0, count: 0 }]);
    const hunk = docs.hunks[0]!;
    expect(sideTone(hunk.theirs, ["a", "b"])).toBe("add");
    expect(sideTone(hunk.ours, ["a", "b"])).toBeNull();
    expect(hunkLegend(hunk, false)).toMatch(/Theirs deleted/);
  });

  it("add/add with other content: one conflict with no base, written with its markers", () => {
    const docs = solverDocs({
      base: null,
      ours: "mine\n",
      theirs: "yours\n",
      regions: [region("conflict", { ours: ["mine"], theirs: ["yours"] })],
    });
    expect(docs.hunks.map((hunk) => hunk.kind)).toEqual(["conflict"]);
    expect(docs.resultText).toBe("<<<<<<< ours\nmine\n=======\nyours\n>>>>>>> theirs\n");
  });

  it("a conflict both sides changed in a long file folds only the lines all three share", () => {
    const head = many("h", 30);
    const tail = many("t", 30);
    const regions = [
      region("equal", { base: head, ours: head, theirs: head, result: head }),
      region("conflict", { base: ["x"], ours: ["X1"], theirs: ["X2", "X3"] }),
      region("equal", { base: tail, ours: tail, theirs: tail, result: tail }),
    ];
    const docs = solverDocs({ base: "", ours: "", theirs: "", regions });
    const [ours, result, theirs] = foldsAt(docs);
    expect(ours).toEqual([
      { gap: 0, from: 0, to: 27 },
      { gap: 1, from: 34, to: 61 },
    ]);
    expect(result?.map((fold) => fold.gap)).toEqual([0, 1]);
    expect(theirs?.[1]).toEqual({ gap: 1, from: 35, to: 62 });
  });

  it("a pair with no difference: the side the Result already is has no tone, the other has", () => {
    const docs = solverDocs({
      base: "a\n",
      ours: "a\n",
      theirs: "b\n",
      regions: [region("theirs", { base: ["a"], ours: ["a"], theirs: ["b"], result: ["b"] })],
    });
    const hunk = docs.hunks[0]!;
    expect(hunkTone(hunk, "ours", false, new Set(), ["b"])).toBe("changed");
    expect(hunkTone(hunk, "theirs", false, new Set(), ["b"])).toBeNull();
  });

  it("does not fold a small file at all", () => {
    const head = many("h", 10);
    const regions = [
      region("equal", { base: head, ours: head, theirs: head, result: head }),
      region("ours", { base: ["x"], ours: ["y"], theirs: ["x"], result: ["y"] }),
    ];
    expect(foldsAt(solverDocs({ base: "", ours: "", theirs: "", regions }))).toEqual([[], [], []]);
  });

  it("a file both sides deleted has nothing to do", () => {
    const docs = solverDocs({ base: "a\n", ours: null, theirs: null, regions: [] });
    expect(docs.hunks).toEqual([]);
    expect(docs.resultText).toBe("");
  });
});

describe("sideTone", () => {
  it("is how the Result differs from a side: added, deleted, changed or nothing", () => {
    expect(sideTone([], ["a"])).toBe("add");
    expect(sideTone(["a"], [])).toBe("del");
    expect(sideTone(["a"], ["b"])).toBe("changed");
    expect(sideTone(["a"], ["a"])).toBeNull();
  });
});

describe("hunkTone", () => {
  const conflict = { id: 1, kind: "conflict", base: ["a"], ours: ["b"], theirs: ["c"] } as unknown as Hunk;
  const fromOurs = { id: 2, kind: "ours", base: ["a"], ours: ["b"], theirs: ["a"] } as unknown as Hunk;

  it("gives a conflict its own color, undecided or decided", () => {
    expect(hunkTone(conflict, "ours", false, new Set([1]))).toBe("conflict");
    expect(hunkTone(conflict, "theirs", true, new Set())).toBe("resolved");
  });

  it("colors a one-sided change as the pane does", () => {
    expect(hunkTone(fromOurs, "ours", true, new Set())).toBe("changed");
    expect(hunkTone(fromOurs, "theirs", true, new Set())).toBeNull();
  });

  it("explains each kind in the legend", () => {
    expect(hunkLegend(conflict, true)).toMatch(/^Conflict/);
    expect(hunkLegend(fromOurs, false)).toMatch(/only Ours/);
  });
});

describe("paneTitle", () => {
  const target = { role: "Rebase target", oid: "e059661", subject: "Master: base file" };

  it("names the role, the side and the commit, the subject only in the tooltip", () => {
    expect(paneTitle(target, "ours", false)).toBe('Rebase target ("ours", e059661)');
    expect(paneTooltip(target, "ours", false)).toBe('Rebase target ("ours", e059661)\nMaster: base file');
  });

  it("says a side that lost the file is deleted, and leaves out a commit it does not have", () => {
    expect(paneTitle(target, "ours", true)).toBe('Rebase target ("ours", e059661) — deleted');
    expect(paneTitle({ role: "Stash", oid: null, subject: null }, "theirs", false)).toBe('Stash ("theirs")');
  });
});

describe("takeChanges", () => {
  it("modify/delete: only taking the empty side changes anything", () => {
    const hunk = solverDocs({ base: "s\n", ours: null, theirs: "s\nx\n", regions: [] }).hunks[0]!;
    expect(takeChanges(hunk, ["s", "x"])).toEqual({
      ours: true,
      theirs: false,
      oursTheirs: false,
      theirsOurs: false,
      base: true,
    });
  });
});
