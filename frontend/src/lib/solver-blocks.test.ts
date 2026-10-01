import { EditorState } from "@codemirror/state";
import { history, redo, undo } from "@codemirror/commands";
import { describe, expect, it } from "vitest";
import type { SolverRegion } from "./ipc";
import {
  blockLines,
  blockRows,
  blockTracker,
  blocksOf,
  initialBlocks,
  takeTransaction,
} from "./solver-blocks";
import { buildResult, isUnresolved } from "./solver-model";

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
