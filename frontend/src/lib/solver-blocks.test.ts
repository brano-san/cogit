import { EditorState } from "@codemirror/state";
import { history, redo, undo } from "@codemirror/commands";
import { describe, expect, it } from "vitest";
import type { SolverRegion } from "./ipc";
import {
  blockLines,
  blockRows,
  blockTracker,
  blocksOf,
  gitLines,
  initialBlocks,
  takeTransaction,
} from "./solver-blocks";
import { buildResult, composeSave, isUnresolved, textToLines } from "./solver-model";

function region(kind: SolverRegion["kind"], parts: Partial<Omit<SolverRegion, "kind">> = {}): SolverRegion {
  return { kind, base: [], ours: [], theirs: [], result: [], ...parts };
}

const same = (line: string) => region("equal", { base: [line], ours: [line], theirs: [line], result: [line] });

// a / conflict(b → ours B, theirs b2 b3) / c / ours-only(d → D) / e
const REGIONS: SolverRegion[] = [
  same("a"),
  region("conflict", { base: ["b"], ours: ["B"], theirs: ["b2", "b3"], result: ["b"] }),
  same("c"),
  region("ours", { base: ["d"], ours: ["D"], theirs: ["d"], result: ["D"] }),
  same("e"),
];

function opened() {
  const built = buildResult(REGIONS);
  const state = EditorState.create({
    doc: built.text,
    extensions: [history(), blockTracker(initialBlocks(built.text, built.spans))],
  });
  return { state, hunks: built.hunks, built };
}

function typed(state: EditorState, from: number, to: number, insert: string): EditorState {
  return state.update({ changes: { from, to, insert }, userEvent: "input.type" }).state;
}

describe("where the hunks start out", () => {
  it("are the character ranges of their lines, newlines included", () => {
    const { state, built } = opened();
    expect(built.text).toBe("a\nb\nc\nD\ne\n");
    expect(blocksOf(state)).toEqual([
      { id: 1, from: 2, to: 4 },
      { id: 3, from: 6, to: 8 },
    ]);
  });

  it("know their lines and where those sit", () => {
    const { state } = opened();
    expect(blockLines(state, blocksOf(state)[0]!)).toEqual(["b"]);
    expect(blockRows(state)).toEqual([
      { start: 1, count: 1 },
      { start: 3, count: 1 },
    ]);
  });
});

describe("while the user types", () => {
  it("text typed inside a hunk stays in it, and the hunks below move down", () => {
    const { state } = opened();
    const next = typed(state, 3, 3, "xx");
    expect(blocksOf(next)).toEqual([
      { id: 1, from: 2, to: 6 },
      { id: 3, from: 8, to: 10 },
    ]);
  });

  it("a new line typed at the end of a hunk's last line is in it", () => {
    const { state } = opened();
    const next = typed(state, 3, 3, "\nmore");
    expect(blockRows(next)[0]).toEqual({ start: 1, count: 2 });
  });

  it("text typed at the start of the line after a hunk is not", () => {
    const { state } = opened();
    const next = typed(state, 4, 4, "new ");
    expect(blocksOf(next)[0]).toEqual({ id: 1, from: 2, to: 4 });
  });

  it("text typed at the start of a hunk's first line is in it", () => {
    const { state } = opened();
    const next = typed(state, 2, 2, "new ");
    expect(blocksOf(next)[0]).toEqual({ id: 1, from: 2, to: 8 });
  });

  it("a hunk whose lines are all deleted is an empty block, and typing there fills it", () => {
    const { state } = opened();
    const emptied = typed(state, 2, 4, "");
    expect(blocksOf(emptied)[0]).toEqual({ id: 1, from: 2, to: 2 });
    expect(blockRows(emptied)[0]).toEqual({ start: 1, count: 0 });
    const filled = typed(emptied, 2, 2, "back\n");
    expect(blocksOf(filled)[0]).toEqual({ id: 1, from: 2, to: 7 });
  });
});

describe("take actions", () => {
  it("replace the hunk's lines and keep tracking it, with the hunks below shifted", () => {
    const { state, hunks } = opened();
    const next = state.update(takeTransaction(state, hunks[0]!, "theirs")).state;
    expect(next.doc.toString()).toBe("a\nb2\nb3\nc\nD\ne\n");
    expect(blocksOf(next)).toEqual([
      { id: 1, from: 2, to: 8 },
      { id: 3, from: 10, to: 12 },
    ]);
  });

  it("both sides, in the order asked", () => {
    const { state, hunks } = opened();
    const next = state.update(takeTransaction(state, hunks[0]!, "theirsOurs")).state;
    expect(next.doc.toString()).toBe("a\nb2\nb3\nB\nc\nD\ne\n");
  });

  it("resolve the conflict, and Undo makes it a conflict again with its range back", () => {
    const { state, hunks } = opened();
    const [conflict] = hunks;
    expect(isUnresolved(conflict!, blockLines(state, blocksOf(state)[0]!))).toBe(true);

    const taken = state.update(takeTransaction(state, conflict!, "ours")).state;
    expect(isUnresolved(conflict!, blockLines(taken, blocksOf(taken)[0]!))).toBe(false);

    let undone: EditorState = taken;
    undo({ state: taken, dispatch: (tr) => (undone = tr.state) });
    expect(undone.doc.toString()).toBe("a\nb\nc\nD\ne\n");
    expect(blocksOf(undone)).toEqual(blocksOf(state));
    expect(isUnresolved(conflict!, blockLines(undone, blocksOf(undone)[0]!))).toBe(true);

    let redone: EditorState = undone;
    redo({ state: undone, dispatch: (tr) => (redone = tr.state) });
    expect(redone.doc.toString()).toBe("a\nB\nc\nD\ne\n");
    expect(blocksOf(redone)[0]).toEqual({ id: 1, from: 2, to: 4 });
  });

  it("taking a side that is empty leaves an empty block", () => {
    const built = buildResult([
      same("a"),
      region("theirs", { base: ["x"], ours: ["x"], theirs: [], result: [] }),
      same("z"),
    ]);
    const state = EditorState.create({
      doc: built.text,
      extensions: [history(), blockTracker(initialBlocks(built.text, built.spans))],
    });
    expect(blocksOf(state)).toEqual([{ id: 1, from: 2, to: 2 }]);
    const next = state.update(takeTransaction(state, built.hunks[0]!, "ours")).state;
    expect(next.doc.toString()).toBe("a\nx\nz\n");
    expect(blocksOf(next)).toEqual([{ id: 1, from: 2, to: 4 }]);
  });
});

// A block that starts or ends in the middle of a line (a newline deleted beside it) was
// counted unresolved by its characters yet written as edited by its lines: the dialog
// promised markers and the base went out.
describe("a block whose edge a deleted newline moved into a line", () => {
  const spans = (state: EditorState) =>
    blockRows(state).map((row, at) => ({ id: blocksOf(state)[at]!.id, start: row.start, count: row.count }));
  const saved = (state: EditorState, built: ReturnType<typeof buildResult>) =>
    composeSave(textToLines(state.doc.toString()), spans(state), built.hunks, { ours: "o", theirs: "t" });

  for (const [name, from, to] of [
    ["before it", 1, 2],
    ["after it", 3, 4],
  ] as const) {
    it(`counts it as edited once the newline ${name} is deleted, and writes no markers`, () => {
      const { state, hunks, built } = opened();
      const next = typed(state, from, to, "");

      expect(isUnresolved(hunks[0]!, blockLines(next, blocksOf(next)[0]!))).toBe(false);
      expect(saved(next, built)).not.toContain("<<<<<<<");
    });
  }

  it("is unresolved while it is untouched, and then gets markers", () => {
    const { state, hunks, built } = opened();
    expect(isUnresolved(hunks[0]!, blockLines(state, blocksOf(state)[0]!))).toBe(true);
    expect(saved(state, built)).toContain("<<<<<<< o");
  });
});

// CodeMirror splits on a lone CR by default; the engine and the block offsets do not.
describe("a line with a lone CR in it", () => {
  it("stays one line, and its conflict stays unresolved", () => {
    const built = buildResult([
      same("a"),
      region("conflict", { base: ["x\ry"], ours: ["X"], theirs: ["Y"], result: ["x\ry"] }),
    ]);
    const state = EditorState.create({
      doc: built.text,
      extensions: [gitLines, history(), blockTracker(initialBlocks(built.text, built.spans))],
    });

    expect(state.doc.toString()).toContain("\r");
    expect(state.doc.lines).toBe(3);
    expect(isUnresolved(built.hunks[0]!, blockLines(state, blocksOf(state)[0]!))).toBe(true);
  });

  it("does not move the block offsets when a line ends in CR", () => {
    const built = buildResult([
      same("a\r"),
      region("conflict", { base: ["b"], ours: ["B"], theirs: ["C"], result: ["b"] }),
    ]);
    const state = EditorState.create({
      doc: built.text,
      extensions: [gitLines, blockTracker(initialBlocks(built.text, built.spans))],
    });

    expect(blockRows(state)).toEqual([{ start: 1, count: 1 }]);
  });
});
