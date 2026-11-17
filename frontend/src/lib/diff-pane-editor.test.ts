import { EditorState, type Text } from "@codemirror/state";
import type { DecorationSet } from "@codemirror/view";
import { describe, expect, it } from "vitest";
import { buildPane, lineChange, padPane, replacement, type CodeRow, type DeletedRow, type EditRow } from "./diff-pane-editor";

function file(lines: number): Text {
  return EditorState.create({ doc: Array.from({ length: lines }, (_, i) => `line ${i + 1}`).join("\n") }).doc;
}

function code(line: number, extra: Partial<CodeRow> = {}): EditRow {
  return { kind: "code", line, tone: "", old: null, flash: false, marks: [], ...extra };
}

function deleted(old: number): DeletedRow {
  return { kind: "deleted", old, tone: "del", flash: false, pieces: [{ text: "gone", cls: "" }] };
}

const fold = (from: number, hidden: number): EditRow => ({ kind: "fold", from, hidden, id: `${from}`, render: () => () => {} });

interface Placed {
  from: number;
  to: number;
  height: number;
  /** Before the position (a widget) or a replaced range: < 0. */
  side: number;
}

function placed(set: DecorationSet): Placed[] {
  const out: Placed[] = [];
  set.between(0, Number.MAX_SAFE_INTEGER, (from, to, deco) => {
    const widget = deco.spec.widget as { estimatedHeight: number };
    out.push({ from, to, height: widget.estimatedHeight, side: deco.startSide });
  });
  return out;
}

describe("the editable pane on the diff's row grid", () => {
  it("folds whole lines into one row", () => {
    const doc = file(10);
    const pane = buildPane(doc, { rows: [code(1), code(2), fold(3, 5), code(8), code(9), code(10)] });

    expect(placed(pane.blocks)).toEqual([{ from: doc.line(3).from, to: doc.line(7).to, height: 18, side: expect.any(Number) }]);
  });

  it("puts filler before the line after it, a row each", () => {
    const doc = file(3);
    const pane = buildPane(doc, { rows: [code(1), { kind: "filler" }, { kind: "filler" }, code(2), code(3)] });

    const fillers = placed(pane.blocks);
    expect(fillers).toHaveLength(2);
    for (const filler of fillers) {
      expect(filler).toMatchObject({ from: doc.line(2).from, to: doc.line(2).from, height: 18 });
      expect(filler.side).toBeLessThan(0);
    }
  });

  it("draws each deleted line as its own widget, and those after the last line at the end", () => {
    const doc = file(2);
    const rows = [code(1), deleted(2), deleted(3), code(2), deleted(5)];
    const pane = buildPane(doc, { rows });

    const [first, second, after] = placed(pane.blocks);
    expect(first).toMatchObject({ from: doc.line(2).from, height: 18 });
    expect(second).toMatchObject({ from: doc.line(2).from, height: 18 });
    expect(after).toMatchObject({ from: doc.length, height: 18 });
    expect(after!.side).toBeGreaterThan(0);
  });

  it("puts a strip that hides nothing before its line, or after the last one", () => {
    const doc = file(3);
    const pane = buildPane(doc, { rows: [code(1), fold(2, 0), code(2), code(3), fold(4, 0)] });

    const [above, after] = placed(pane.blocks);
    expect(above).toMatchObject({ from: doc.line(2).from, to: doc.line(2).from, height: 18 });
    expect(above!.side).toBeLessThan(0);
    expect(after).toMatchObject({ from: doc.length, to: doc.length, height: 18 });
    expect(after!.side).toBeGreaterThan(0);
  });

  it("skips rows past the text: the next diff puts them right", () => {
    const doc = file(3);
    const pane = buildPane(doc, { rows: [code(1), code(5), fold(3, 4)] });

    expect(pane.lines.size).toBe(1);
    expect(placed(pane.blocks)).toEqual([]);
  });

  it("keeps each line's row for the gutters, and its marks inside the line", () => {
    const doc = file(2);
    const row = code(2, { tone: "add", marks: [{ from: 2, to: 99, cls: "word" }] });
    const pane = buildPane(doc, { rows: [code(1), row] });

    const lines: unknown[] = [];
    pane.lines.between(0, doc.length, (from, _to, deco) => void lines.push([from, (deco.spec as { row: EditRow }).row]));
    expect(lines).toEqual([
      [0, code(1)],
      [doc.line(2).from, row],
    ]);
    const words: [number, number][] = [];
    pane.words.between(0, doc.length, (from, to) => void words.push([from, to]));
    expect(words).toEqual([[doc.line(2).from + 2, doc.line(2).to]]);
  });
});

describe("padPane: wrapped Aligned (F-950)", () => {
  it("puts filler under a line shorter than the one opposite, and sizes the filler rows", () => {
    const doc = file(3);
    const rows: EditRow[] = [code(1), { kind: "filler" }, { kind: "filler" }, code(2), code(3)];
    const pads = Float64Array.from([36, 54, 18, 0, 0]);
    const pane = padPane(doc, buildPane(doc, { rows }), pads);

    expect(placed(pane.blocks)).toMatchObject([
      { from: doc.line(2).from, height: 54 },
      { from: doc.line(2).from, height: 18 },
    ]);
    const [under] = placed(pane.under);
    expect(under).toMatchObject({ from: doc.line(1).to, height: 36 });
    expect(under!.side).toBeGreaterThan(0);
  });

  it("goes back to one row per filler without pads", () => {
    const doc = file(2);
    const rows: EditRow[] = [code(1), { kind: "filler" }, code(2)];
    const padded = padPane(doc, buildPane(doc, { rows }), Float64Array.from([18, 90, 0]));
    const plain = padPane(doc, padded, null);

    expect(placed(plain.blocks)).toMatchObject([{ height: 18 }]);
    expect(plain.under.size).toBe(0);
  });
});

describe("replacement", () => {
  it("changes only the part that differs", () => {
    expect(replacement("abcdef", "abXYef")).toEqual({ from: 2, to: 4, insert: "XY" });
    expect(replacement("one\ntwo", "one\ntwo\nthree")).toEqual({ from: 7, to: 7, insert: "\nthree" });
    expect(replacement("abc", "")).toEqual({ from: 0, to: 3, insert: "" });
  });

  it("is nothing for the same text", () => {
    expect(replacement("same", "same")).toBeNull();
  });
});

describe("lineChange", () => {
  const after = (text: string, from: number, now: string[], then: string[]) => {
    const state = EditorState.create({ doc: text });
    const change = lineChange(state.doc, from, now, then);
    return change && state.update({ changes: change }).newDoc.toString();
  };

  it("takes a block of the diff back to its old lines", () => {
    expect(after("a\nX\nY\nc\n", 2, ["X", "Y"], ["b"])).toBe("a\nb\nc\n");
    expect(after("a\nX\nc\n", 2, ["X"], [])).toBe("a\nc\n");
    expect(after("a\nc\n", 2, [], ["b"])).toBe("a\nb\nc\n");
  });

  it("keeps the end of the file as the diff counts it", () => {
    // A line after the last line break, and the break itself (the engine's empty last line).
    expect(after("a\nb\nX", 3, ["X"], [])).toBe("a\nb");
    expect(after("a\nb\n", 3, [""], [])).toBe("a\nb");
    expect(after("a\nb", 3, [], [""])).toBe("a\nb\n");
    expect(after("a\nb", 3, [], ["c"])).toBe("a\nb\nc");
  });

  it("is nothing once the lines read otherwise: typed since the diff", () => {
    expect(after("a\nXY\nc", 2, ["X"], ["b"])).toBeNull();
    expect(after("a", 3, ["X"], ["b"])).toBeNull();
  });
});
