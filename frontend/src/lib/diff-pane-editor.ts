import { syntaxHighlighting } from "@codemirror/language";
import { Compartment, EditorState, StateEffect, StateField, type Extension, type Range, type Text } from "@codemirror/state";
import { Decoration, EditorView, GutterMarker, WidgetType, gutter, type DecorationSet } from "@codemirror/view";
import { classHighlighter } from "@lezer/highlight";
import { TAB_SIZE } from "$lib/code-scroll";
import { editing, type EditEvents } from "$lib/diff-editor";
import { Pad, styleNonce } from "$lib/solver-editor";
import { LINE_HEIGHT as ROW } from "$lib/solver-geometry";

/**
 * The working file as a pane of its diff (F-722): CodeMirror, laid out on the diff's own row
 * grid, so the band, the scroll sync and the left pane go on working from row arithmetic.
 * Every row is 18 px: a line of the file, a fold strip in place of the unchanged lines it
 * hides, filler opposite lines only the other side has (Aligned), or, in Unified, the
 * deleted lines between the lines of the file. The rows come from the diff of the text last
 * sent; typing carries them along with the text until the next diff arrives.
 */

export type Tone = "del" | "add" | "changed" | "move" | "";
export type Pick = "staging" | "marked" | null;
export type PaneLayout = "split" | "unified";

export interface Mark {
  from: number;
  to: number;
  cls: string;
}

/** A line of the file. */
export interface CodeRow {
  kind: "code";
  /** 1-based, in the file as sent. */
  line: number;
  tone: Tone;
  /** Unified: the old number of an unchanged line. */
  old: number | null;
  key: string | null;
  pick: Pick;
  flash: boolean;
  marks: readonly Mark[];
}

/** Unified: a line of the old side only, drawn between the lines of the file. */
export interface DeletedRow {
  kind: "deleted";
  old: number;
  tone: Tone;
  key: string | null;
  pick: Pick;
  flash: boolean;
  pieces: readonly { text: string; cls: string }[];
}

/** Unchanged lines folded into one strip. Rows with the same `id` draw the same strip. */
export interface FoldRow {
  kind: "fold";
  /** The first line of the file it hides. */
  from: number;
  hidden: number;
  id: string;
  /** Draws the strip into `host`; returns how to take it down. */
  render: (host: HTMLElement) => () => void;
}

export interface FillerRow {
  kind: "filler";
}

export type EditRow = CodeRow | DeletedRow | FoldRow | FillerRow;

export interface PaneRows {
  rows: readonly EditRow[];
  /** Lines can be staged: a changed line shows its box. */
  boxes: boolean;
}

export interface PaneEvents extends EditEvents {
  /** A box in the gutter was clicked. */
  pick: (key: string) => void;
  /** Where the text is scrolled sideways and how far it can go: told when the caret scrolls
      it past an edge, and when a line typed longer or shorter changes the reach. */
  sideways: (left: number, overflow: number) => void;
}

// --- the rows as decorations ---

const unmounts = new WeakMap<HTMLElement, () => void>();

class Fold extends WidgetType {
  constructor(readonly row: FoldRow) {
    super();
  }

  override eq(other: Fold): boolean {
    return other.row.id === this.row.id;
  }

  override toDOM(): HTMLElement {
    const host = document.createElement("div");
    host.className = "dv-fold-row";
    unmounts.set(host, this.row.render(host));
    return host;
  }

  override destroy(dom: HTMLElement): void {
    unmounts.get(dom)?.();
    unmounts.delete(dom);
  }

  override get estimatedHeight(): number {
    return ROW;
  }

  override ignoreEvent(): boolean {
    return true;
  }
}

class Deleted extends WidgetType {
  readonly #sig: string;

  constructor(readonly rows: readonly DeletedRow[]) {
    super();
    this.#sig = JSON.stringify(rows.map((row) => [row.old, row.tone, row.pick, row.flash, row.pieces]));
  }

  override eq(other: Deleted): boolean {
    return other.#sig === this.#sig;
  }

  override toDOM(): HTMLElement {
    const box = document.createElement("div");
    box.className = "dv-deleted";
    for (const row of this.rows) {
      const line = document.createElement("div");
      line.className = classes("dv-del-line", row.tone, row.flash && "flash");
      for (const piece of row.pieces) {
        const span = document.createElement("span");
        if (piece.cls) span.className = piece.cls;
        span.textContent = piece.text;
        line.append(span);
      }
      box.append(line);
    }
    return box;
  }

  override get estimatedHeight(): number {
    return this.rows.length * ROW;
  }

  override ignoreEvent(): boolean {
    return true;
  }
}

interface Pane {
  /** Line decorations; each carries its `CodeRow` for the gutters. */
  lines: DecorationSet;
  blocks: DecorationSet;
  words: DecorationSet;
  boxes: boolean;
}

const EMPTY: Pane = { lines: Decoration.none, blocks: Decoration.none, words: Decoration.none, boxes: false };

export const setRows = StateEffect.define<PaneRows>();

/** Class names are the diff view's own (`num`, `sign`, `del`, `word`…): one stylesheet draws
    its HTML rows and these. */
function classes(...names: (string | false | null | undefined)[]): string {
  return names.filter(Boolean).join(" ");
}

/** Rows that point past the text (a diff of text since shortened) are skipped: the next
    diff puts them right. */
export function buildPane(doc: Text, { rows, boxes }: PaneRows): Pane {
  const lines: Range<Decoration>[] = [];
  const blocks: Range<Decoration>[] = [];
  const words: Range<Decoration>[] = [];
  let deleted: DeletedRow[] = [];
  let fillers = 0;

  /** Deleted lines and filler go before the line that follows them, or after the last one. */
  const place = (before: number | null) => {
    if (deleted.length === 0 && fillers === 0) return;
    const widget = deleted.length > 0 ? new Deleted(deleted) : new Pad(fillers, "plain", false);
    const inside = before !== null && before >= 1 && before <= doc.lines;
    blocks.push(
      Decoration.widget({ widget, block: true, side: inside ? -1 : 1 }).range(inside ? doc.line(before).from : doc.length),
    );
    deleted = [];
    fillers = 0;
  };

  for (const row of rows) {
    if (row.kind === "deleted") {
      if (fillers > 0) place(null);
      deleted.push(row);
    } else if (row.kind === "filler") {
      if (deleted.length > 0) place(null);
      fillers += 1;
    } else if (row.kind === "fold") {
      const last = row.from + row.hidden - 1;
      if (row.from < 1 || last > doc.lines) continue;
      place(row.from);
      const range = Decoration.replace({ widget: new Fold(row), block: true });
      blocks.push(range.range(doc.line(row.from).from, doc.line(last).to));
    } else {
      if (row.line < 1 || row.line > doc.lines) continue;
      place(row.line);
      const line = doc.line(row.line);
      lines.push(Decoration.line({ class: classes(row.tone, row.flash && "flash"), row }).range(line.from));
      for (const mark of row.marks) {
        const from = line.from + Math.min(mark.from, line.length);
        const to = line.from + Math.min(mark.to, line.length);
        if (to > from) words.push(Decoration.mark({ class: mark.cls }).range(from, to));
      }
    }
  }
  place(null);
  return {
    lines: Decoration.set(lines, true),
    blocks: Decoration.set(blocks, true),
    words: Decoration.set(words, true),
    boxes,
  };
}

const paneField = StateField.define<Pane>({
  create: () => EMPTY,
  update(pane, tr) {
    for (const effect of tr.effects) if (effect.is(setRows)) return buildPane(tr.state.doc, effect.value);
    if (!tr.docChanged) return pane;
    return {
      lines: pane.lines.map(tr.changes),
      blocks: pane.blocks.map(tr.changes),
      words: pane.words.map(tr.changes),
      boxes: pane.boxes,
    };
  },
  provide: (field) => [
    EditorView.decorations.from(field, (pane) => pane.lines),
    EditorView.decorations.from(field, (pane) => pane.blocks),
    EditorView.decorations.from(field, (pane) => pane.words),
  ],
});

/** The row a line of the file was drawn from; none for a line typed since the last diff. */
export function rowAt(state: EditorState, from: number): CodeRow | null {
  let found: CodeRow | null = null;
  state.field(paneField).lines.between(from, from, (start, _to, deco) => {
    if (start !== from) return;
    found = (deco.spec as { row: CodeRow }).row;
    return false;
  });
  return found;
}

// --- the gutters: numbers and signs before the text, the boxes after it (side by side) ---

interface Cell {
  cls: string;
  text: string;
  key?: string | null;
}

class Cells extends GutterMarker {
  readonly #sig: string;

  /** One list of cells per row: a run of deleted lines takes several. */
  constructor(
    readonly rows: readonly (readonly Cell[])[],
    readonly rowClasses: readonly string[],
  ) {
    super();
    this.#sig = JSON.stringify([rows, rowClasses]);
  }

  override eq(other: Cells): boolean {
    return other.#sig === this.#sig;
  }

  override toDOM(): Node {
    const box = document.createDocumentFragment();
    this.rows.forEach((cells, index) => {
      const row = document.createElement("div");
      row.className = classes("dv-cells", this.rowClasses[index]);
      for (const cell of cells) {
        const span = document.createElement("span");
        span.className = cell.cls;
        span.textContent = cell.text;
        if (cell.key) span.dataset.key = cell.key;
        row.append(span);
      }
      box.append(row);
    });
    return box;
  }
}

class Strip extends GutterMarker {
  constructor(readonly cls: string) {
    super();
  }

  override eq(other: Strip): boolean {
    return other.cls === this.cls;
  }

  override toDOM(): Node {
    const strip = document.createElement("div");
    strip.className = this.cls;
    return strip;
  }
}

const FOLD_STRIP = new Strip("dv-gutter-fold");
const FILLER_STRIP = new Strip("dv-gutter-filler");

function rowClass(pick: Pick, flash: boolean): string {
  return classes(pick, flash && "flash");
}

function box(key: string | null, pick: Pick, boxes: boolean): Cell {
  return { cls: classes("gutter", pick && "picked"), text: boxes && key ? (pick ? "■" : "□") : "", key };
}

function num(value: number | null, tones: string): Cell {
  return { cls: classes("num", tones), text: value === null ? "" : String(value) };
}

function lineCells(state: EditorState, from: number, layout: PaneLayout, after: boolean): GutterMarker {
  const row = rowAt(state, from);
  const tones = row?.tone ?? "";
  const pick = row?.pick ?? null;
  const sel = box(row?.key ?? null, pick, state.field(paneField).boxes);
  const sign = { cls: classes("sign", tones), text: tones ? "+" : "" };
  const line = state.doc.lineAt(from).number;
  const cells: Cell[] =
    layout === "unified" ? [sel, num(row?.old ?? null, tones), num(line, tones), sign] : after ? [sel] : [num(line, tones), sign];
  return new Cells([cells], [rowClass(pick, row?.flash ?? false)]);
}

function widgetCells(state: EditorState, widget: WidgetType, layout: PaneLayout): GutterMarker | null {
  if (widget instanceof Fold) return FOLD_STRIP;
  if (widget instanceof Pad) return FILLER_STRIP;
  if (!(widget instanceof Deleted) || layout !== "unified") return null;
  const boxes = state.field(paneField).boxes;
  return new Cells(
    widget.rows.map((row) => [
      box(row.key, row.pick, boxes),
      num(row.old, row.tone),
      num(null, row.tone),
      { cls: classes("sign", row.tone), text: "−" },
    ]),
    widget.rows.map((row) => rowClass(row.pick, row.flash)),
  );
}

function gutters(layout: PaneLayout, pick: (key: string) => void): Extension {
  const side = (after: boolean) =>
    gutter({
      class: after ? "dv-gutter dv-gutter-after" : "dv-gutter",
      side: after ? "after" : "before",
      lineMarker: (view, line) => lineCells(view.state, line.from, layout, after),
      widgetMarker: (view, widget) => widgetCells(view.state, widget, layout),
      lineMarkerChange: (update) => update.transactions.some((tr) => tr.effects.some((effect) => effect.is(setRows))),
      domEventHandlers: {
        mousedown: (_view, _line, event) => {
          const key = (event.target as HTMLElement | null)?.closest<HTMLElement>("[data-key]")?.dataset.key;
          if (!key) return false;
          event.preventDefault();
          pick(key);
          return true;
        },
      },
    });
  return layout === "unified" ? side(false) : [side(false), side(true)];
}

/** Only what the row grid needs; the look is the diff view's stylesheet. */
const grid = EditorView.theme({
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { lineHeight: `${ROW}px`, overflowX: "auto", overflowY: "hidden" },
  ".cm-content": { padding: "0" },
  ".cm-line": { height: `${ROW}px`, padding: "0" },
  ".dv-gutter .cm-gutterElement": { padding: "0", minWidth: "0" },
  ".dv-cells": { display: "flex", height: `${ROW}px`, lineHeight: `${ROW}px` },
  ".dv-fold-row": { display: "flex", height: `${ROW}px` },
  ".dv-del-line": { height: `${ROW}px`, lineHeight: `${ROW}px`, whiteSpace: "pre" },
});

/** The text, from `old` to `next`, as one change of the part that differs: the caret and
    the scroll stay where they were, and Undo brings the old text back. */
export function replacement(old: string, next: string): { from: number; to: number; insert: string } | null {
  if (old === next) return null;
  let start = 0;
  const shorter = Math.min(old.length, next.length);
  while (start < shorter && old.charCodeAt(start) === next.charCodeAt(start)) start += 1;
  let end = 0;
  while (end < shorter - start && old.charCodeAt(old.length - 1 - end) === next.charCodeAt(next.length - 1 - end)) end += 1;
  return { from: start, to: old.length - end, insert: next.slice(start, next.length - end) };
}

export class DiffPaneEditor {
  readonly view: EditorView;
  #saved: Text;
  #layout: PaneLayout;
  #gutters = new Compartment();
  #language = new Compartment();
  #events: PaneEvents;
  #resized: ResizeObserver;
  #destroyed = false;

  /** `saved`: the text on disk, when the editor opens on edits made before (the pane was
      drawn again); the editor is dirty from the start then. */
  constructor(text: string, layout: PaneLayout, events: PaneEvents, saved: string = text) {
    this.#layout = layout;
    this.#events = events;
    this.view = new EditorView({
      state: EditorState.create({
        doc: text,
        extensions: [
          styleNonce(),
          grid,
          EditorState.tabSize.of(TAB_SIZE),
          paneField,
          this.#gutters.of(gutters(layout, events.pick)),
          this.#language.of([]),
          syntaxHighlighting(classHighlighter),
          editing(events),
        ],
      }),
    });
    this.#saved = saved === text ? this.view.state.doc : EditorState.create({ doc: saved }).doc;
    let left = 0;
    let overflow = 0;
    const scroller = this.view.scrollDOM;
    // A line typed longer or shorter changes how far the text scrolls without a scroll.
    const report = () => {
      const now = scroller.scrollLeft;
      const room = scroller.scrollWidth - scroller.clientWidth;
      if (now !== left || room !== overflow) events.sideways((left = now), (overflow = room));
    };
    scroller.addEventListener("scroll", report);
    this.#resized = new ResizeObserver(report);
    this.#resized.observe(this.view.contentDOM);
    this.#resized.observe(scroller);
  }

  /** Into `parent` (the pane of the layout on screen), with that layout's gutters. */
  mount(parent: HTMLElement, layout: PaneLayout): void {
    if (this.view.dom.parentElement !== parent) parent.append(this.view.dom);
    if (layout !== this.#layout) {
      this.#layout = layout;
      this.view.dispatch({ effects: this.#gutters.reconfigure(gutters(layout, this.#events.pick)) });
    }
    this.view.requestMeasure();
  }

  setRows(rows: PaneRows): void {
    this.view.dispatch({ effects: setRows.of(rows) });
  }

  setLanguage(language: Extension | null): void {
    if (!this.#destroyed) this.view.dispatch({ effects: this.#language.reconfigure(language ?? []) });
  }

  /** The code column's own sideways scroll, which the diff moves with its other columns. */
  scrollSideways(left: number): void {
    if (this.view.scrollDOM.scrollLeft !== left) this.view.scrollDOM.scrollLeft = left;
  }

  text(): string {
    return this.view.state.doc.toString();
  }

  get doc(): Text {
    return this.view.state.doc;
  }

  get dirty(): boolean {
    return !this.view.state.doc.eq(this.#saved);
  }

  /** What is on disk now is `doc`, the text the save wrote. */
  markSaved(doc: Text): void {
    this.#saved = doc;
  }

  /** The file as read again from disk; Undo brings the edits back. Clean before the change
      is told: the listener reads `dirty`. */
  replace(text: string): void {
    const change = replacement(this.view.state.doc.toString(), text);
    if (!change) {
      this.#saved = this.view.state.doc;
      return;
    }
    const tr = this.view.state.update({ changes: change });
    this.#saved = tr.newDoc;
    this.view.dispatch(tr);
  }

  get focused(): boolean {
    return this.view.hasFocus;
  }

  destroy(): void {
    this.#destroyed = true;
    this.#resized.disconnect();
    this.view.destroy();
  }
}
