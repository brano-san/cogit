import { syntaxHighlighting } from "@codemirror/language";
import { Compartment, EditorState, StateEffect, StateField, type Extension, type Range, type Text } from "@codemirror/state";
import { Decoration, EditorView, GutterMarker, WidgetType, gutter, type BlockInfo, type DecorationSet } from "@codemirror/view";
import { classHighlighter } from "@lezer/highlight";
import { TAB_SIZE } from "$lib/code-scroll";
import { editing, type EditEvents } from "$lib/diff-editor";
import { styleNonce } from "$lib/solver-editor";
import { LINE_HEIGHT as ROW } from "$lib/solver-geometry";

/**
 * The working file as a pane of its diff (F-722): CodeMirror, laid out on the diff's own row
 * grid. A row is a line of the file, a fold strip in place of the unchanged lines it hides,
 * filler opposite lines only the other side has (Aligned), or, in Unified, a deleted line
 * between the lines of the file. Unwrapped, every row is 18 px; wrapped, row positions come
 * from CodeMirror's height map. The rows come from the diff of the text last
 * sent; typing carries them along with the text until the next diff arrives.
 */

export type Tone = "del" | "add" | "changed" | "move" | "";
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
  flash: boolean;
  marks: readonly Mark[];
}

/** Unified: a line of the old side only, drawn between the lines of the file. */
export interface DeletedRow {
  kind: "deleted";
  old: number;
  tone: Tone;
  flash: boolean;
  pieces: readonly { text: string; cls: string }[];
}

/** Unchanged lines folded into one strip; hiding none, the strip of the block below's actions.
    Rows with the same `id` draw the same strip. */
export interface FoldRow {
  kind: "fold";
  /** The first line of the file it hides, or the line it stands above. */
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
}

export interface PaneEvents extends EditEvents {
  /** Where the text is scrolled sideways and how far it can go: told when the caret scrolls
      it past an edge, and when a line typed longer or shorter changes the reach. */
  sideways: (left: number, overflow: number) => void;
  heights?: () => void;
}

// --- the rows as decorations ---

const unmounts = new WeakMap<HTMLElement, () => void>();

class Fold extends WidgetType {
  constructor(
    readonly row: FoldRow,
    readonly index: number,
  ) {
    super();
  }

  override eq(other: Fold): boolean {
    return other.row.id === this.row.id && other.index === this.index;
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

  constructor(
    readonly rows: readonly DeletedRow[],
    readonly index: number,
  ) {
    super();
    this.#sig = JSON.stringify([index, rows.map((row) => [row.old, row.tone, row.flash, row.pieces])]);
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

export class Room extends WidgetType {
  constructor(
    readonly height: number,
    readonly index: number,
    readonly under: boolean,
  ) {
    super();
  }

  override eq(other: Room): boolean {
    return other.height === this.height && other.index === this.index && other.under === this.under;
  }

  override toDOM(): HTMLElement {
    const pad = document.createElement("div");
    pad.className = "sv-pad";
    pad.style.height = `${this.height}px`;
    pad.setAttribute("aria-hidden", "true");
    return pad;
  }

  override get estimatedHeight(): number {
    return this.height;
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
  under: DecorationSet;
}

const EMPTY: Pane = { lines: Decoration.none, blocks: Decoration.none, words: Decoration.none, under: Decoration.none };

export const setRows = StateEffect.define<PaneRows>();
export const setPads = StateEffect.define<Float64Array | null>();

/** Class names are the diff view's own (`num`, `sign`, `del`, `word`…): one stylesheet draws
    its HTML rows and these. */
function classes(...names: (string | false | null | undefined)[]): string {
  return names.filter(Boolean).join(" ");
}

/** Rows that point past the text (a diff of text since shortened) are skipped: the next
    diff puts them right. */
export function buildPane(doc: Text, { rows }: PaneRows): Pane {
  const lines: Range<Decoration>[] = [];
  const blocks: Range<Decoration>[] = [];
  const words: Range<Decoration>[] = [];
  let deleted: { row: DeletedRow; index: number }[] = [];
  let fillers: number[] = [];

  /** Deleted lines and filler go before the line that follows them, or after the last one. */
  const place = (before: number | null) => {
    if (deleted.length === 0 && fillers.length === 0) return;
    const widgets =
      deleted.length > 0
        ? deleted.map(({ row, index }) => new Deleted([row], index))
        : fillers.map((index) => new Room(ROW, index, false));
    const inside = before !== null && before >= 1 && before <= doc.lines;
    for (const widget of widgets) {
      blocks.push(
        Decoration.widget({ widget, block: true, side: inside ? -1 : 1 }).range(inside ? doc.line(before).from : doc.length),
      );
    }
    deleted = [];
    fillers = [];
  };

  rows.forEach((row, index) => {
    if (row.kind === "deleted") {
      if (fillers.length > 0) place(null);
      deleted.push({ row, index });
    } else if (row.kind === "filler") {
      if (deleted.length > 0) place(null);
      fillers.push(index);
    } else if (row.kind === "fold" && row.hidden === 0) {
      // The strip of a block's actions where nothing is folded: between two lines.
      place(row.from);
      const inside = row.from >= 1 && row.from <= doc.lines;
      const widget = Decoration.widget({ widget: new Fold(row, index), block: true, side: inside ? -1 : 1 });
      blocks.push(widget.range(inside ? doc.line(row.from).from : doc.length));
    } else if (row.kind === "fold") {
      const last = row.from + row.hidden - 1;
      if (row.from < 1 || last > doc.lines) return;
      place(row.from);
      const range = Decoration.replace({ widget: new Fold(row, index), block: true });
      blocks.push(range.range(doc.line(row.from).from, doc.line(last).to));
    } else {
      if (row.line < 1 || row.line > doc.lines) return;
      place(row.line);
      const line = doc.line(row.line);
      lines.push(Decoration.line({ class: classes(row.tone, row.flash && "flash"), row, index }).range(line.from));
      for (const mark of row.marks) {
        const from = line.from + Math.min(mark.from, line.length);
        const to = line.from + Math.min(mark.to, line.length);
        if (to > from) words.push(Decoration.mark({ class: mark.cls }).range(from, to));
      }
    }
  });
  place(null);
  return {
    lines: Decoration.set(lines, true),
    blocks: Decoration.set(blocks, true),
    words: Decoration.set(words, true),
    under: Decoration.none,
  };
}

export function padPane(doc: Text, pane: Pane, pads: Float64Array | null): Pane {
  const blocks: Range<Decoration>[] = [];
  let changed = false;
  for (let at = pane.blocks.iter(); at.value; at.next()) {
    const deco = at.value;
    const widget = (deco.spec as { widget?: unknown }).widget;
    if (widget instanceof Room) {
      const height = pads ? (pads[widget.index] ?? 0) : ROW;
      if (height !== widget.height) {
        changed = true;
        const side = (deco.spec as { side?: number }).side ?? 1;
        blocks.push(Decoration.widget({ widget: new Room(height, widget.index, false), block: true, side }).range(at.from));
        continue;
      }
    }
    blocks.push(deco.range(at.from, at.to));
  }
  const under: Range<Decoration>[] = [];
  if (pads) {
    for (let at = pane.lines.iter(); at.value; at.next()) {
      const index = (at.value.spec as { index: number }).index;
      const height = pads[index] ?? 0;
      if (height <= 0) continue;
      under.push(Decoration.widget({ widget: new Room(height, index, true), block: true, side: 1 }).range(doc.lineAt(at.from).to));
    }
  }
  return { ...pane, blocks: changed ? Decoration.set(blocks, true) : pane.blocks, under: Decoration.set(under, true) };
}

interface Padded extends Pane {
  pads: Float64Array | null;
}

const paneField = StateField.define<Padded>({
  create: () => ({ ...EMPTY, pads: null }),
  update(pane, tr) {
    for (const effect of tr.effects) {
      if (effect.is(setRows)) {
        const built = buildPane(tr.state.doc, effect.value);
        return { ...(pane.pads ? padPane(tr.state.doc, built, pane.pads) : built), pads: pane.pads };
      }
    }
    for (const effect of tr.effects) {
      if (effect.is(setPads)) return { ...padPane(tr.state.doc, pane, effect.value), pads: effect.value };
    }
    if (!tr.docChanged) return pane;
    return {
      lines: pane.lines.map(tr.changes),
      blocks: pane.blocks.map(tr.changes),
      words: pane.words.map(tr.changes),
      under: pane.under.map(tr.changes),
      pads: pane.pads,
    };
  },
  provide: (field) => [
    EditorView.decorations.from(field, (pane) => pane.lines),
    EditorView.decorations.from(field, (pane) => pane.blocks),
    EditorView.decorations.from(field, (pane) => pane.words),
    EditorView.decorations.from(field, (pane) => pane.under),
  ],
});

function rowIndexAt(state: EditorState, from: number): number | null {
  let found: number | null = null;
  state.field(paneField).lines.between(from, from, (start, _to, deco) => {
    if (start !== from) return;
    found = (deco.spec as { index: number }).index;
    return false;
  });
  return found;
}

export interface RowExtents {
  extent: Float64Array;
  natural: Float64Array;
  shown: Uint8Array;
}

export function rowExtents(view: EditorView, count: number): RowExtents {
  const extent = new Float64Array(count);
  const padded = new Float64Array(count);
  const shown = new Uint8Array(count);
  const { from: shownFrom, to: shownTo } = view.viewport;
  const state = view.state;
  let open = -1;
  let openTop = 0;
  let openPad = false;
  let last = -1;
  const close = (top: number) => {
    if (open < 0) return;
    const height = Math.max(top - openTop, 0);
    extent[open] = extent[open]! + height;
    if (openPad) padded[open] = padded[open]! + height;
  };
  const leaf = (block: BlockInfo) => {
    const widget = block.widget;
    let index: number | null = null;
    let pad = false;
    if (widget instanceof Fold || widget instanceof Deleted) index = widget.index;
    else if (widget instanceof Room) {
      index = widget.index;
      pad = true;
    } else if (!widget) index = rowIndexAt(state, block.from);
    if (index === null || index < 0 || index >= count) index = last >= 0 ? last : count > 0 ? 0 : -1;
    if (index >= 0 && !pad && block.from >= shownFrom && block.to <= shownTo) shown[index] = 1;
    close(block.top);
    open = index;
    openTop = block.top;
    openPad = pad;
    last = index;
  };
  const end = state.doc.length;
  let pos = 0;
  let bottom = 0;
  for (;;) {
    const block = view.lineBlockAt(pos);
    const type = block.type;
    if (typeof type === "number") leaf(block);
    else for (const part of type) leaf(part);
    bottom = block.bottom;
    if (block.to >= end) break;
    pos = block.to + 1;
  }
  close(bottom);
  const natural = new Float64Array(count);
  for (let i = 0; i < count; i++) natural[i] = extent[i]! - padded[i]!;
  return { extent, natural, shown };
}

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

// --- the gutter: numbers and signs before the text ---

interface Cell {
  cls: string;
  text: string;
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

function rowClass(flash: boolean): string {
  return classes(flash && "flash");
}

function num(value: number | null, tones: string): Cell {
  return { cls: classes("num", tones), text: value === null ? "" : String(value) };
}

function lineCells(state: EditorState, from: number, layout: PaneLayout): GutterMarker {
  const row = rowAt(state, from);
  const tones = row?.tone ?? "";
  const sign = { cls: classes("sign", tones), text: tones ? "+" : "" };
  const line = state.doc.lineAt(from).number;
  const cells: Cell[] =
    layout === "unified" ? [num(row?.old ?? null, tones), num(line, tones), sign] : [num(line, tones), sign];
  return new Cells([cells], [rowClass(row?.flash ?? false)]);
}

function widgetCells(widget: WidgetType, layout: PaneLayout): GutterMarker | null {
  if (widget instanceof Fold) return FOLD_STRIP;
  if (widget instanceof Room) return FILLER_STRIP;
  if (!(widget instanceof Deleted) || layout !== "unified") return null;
  return new Cells(
    widget.rows.map((row) => [num(row.old, row.tone), num(null, row.tone), { cls: classes("sign", row.tone), text: "−" }]),
    widget.rows.map((row) => rowClass(row.flash)),
  );
}

function gutters(layout: PaneLayout): Extension {
  return gutter({
    class: "dv-gutter",
    lineMarker: (view, line) => lineCells(view.state, line.from, layout),
    widgetMarker: (_view, widget) => widgetCells(widget, layout),
    lineMarkerChange: (update) => update.transactions.some((tr) => tr.effects.some((effect) => effect.is(setRows))),
  });
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

const BREAK_ANYWHERE = {
  whiteSpace: "break-spaces",
  wordBreak: "break-all",
  overflowWrap: "anywhere",
  lineBreak: "anywhere",
};

const wrapped: Extension = [
  EditorView.lineWrapping,
  EditorView.editorAttributes.of({ class: "dv-wrap" }),
  EditorView.theme({
    "&.dv-wrap .cm-scroller": { overflowX: "hidden" },
    "&.dv-wrap .cm-content": BREAK_ANYWHERE,
    "&.dv-wrap .cm-line": { height: "auto", minHeight: `${ROW}px` },
    "&.dv-wrap .dv-del-line": { ...BREAK_ANYWHERE, height: "auto", minHeight: `${ROW}px` },
  }),
];

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

/** Lines `from`… (1-based) that read `now` become `then`; with `now` empty, `then` goes in
    before line `from`. Null when the text no longer reads `now` there: typed since the diff. */
export function lineChange(
  doc: Text,
  from: number,
  now: readonly string[],
  then: readonly string[],
): { from: number; to?: number; insert?: string } | null {
  const last = from + now.length - 1;
  if (from < 1 || last > doc.lines || from > doc.lines + 1) return null;
  if (now.some((text, i) => doc.line(from + i).text !== text)) return null;
  const lines = then.join("\n");
  if (now.length > 0 && then.length > 0) return { from: doc.line(from).from, to: doc.line(last).to, insert: lines };
  // Lines out with a line break: the one after them, or at the end the one before.
  if (now.length > 0 && last < doc.lines) return { from: doc.line(from).from, to: doc.line(last + 1).from };
  if (now.length > 0) return { from: from > 1 ? doc.line(from - 1).to : 0, to: doc.length };
  if (then.length === 0) return null;
  return from <= doc.lines ? { from: doc.line(from).from, insert: `${lines}\n` } : { from: doc.length, insert: `\n${lines}` };
}

export class DiffPaneEditor {
  readonly view: EditorView;
  #saved: Text;
  #layout: PaneLayout;
  #gutters = new Compartment();
  #language = new Compartment();
  #wrap = new Compartment();
  #wrapped = false;
  #resized: ResizeObserver;
  #destroyed = false;

  /** `saved`: the text on disk, when the editor opens on edits made before (the pane was
      drawn again); the editor is dirty from the start then. */
  constructor(text: string, layout: PaneLayout, events: PaneEvents, saved: string = text) {
    this.#layout = layout;
    this.view = new EditorView({
      state: EditorState.create({
        doc: text,
        extensions: [
          styleNonce(),
          grid,
          EditorState.tabSize.of(TAB_SIZE),
          paneField,
          this.#gutters.of(gutters(layout)),
          this.#language.of([]),
          this.#wrap.of([]),
          syntaxHighlighting(classHighlighter),
          editing(events),
          EditorView.updateListener.of((update) => {
            if (update.heightChanged || update.geometryChanged || update.viewportChanged) events.heights?.();
          }),
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
      this.view.dispatch({ effects: this.#gutters.reconfigure(gutters(layout)) });
    }
    this.view.requestMeasure();
  }

  setRows(rows: PaneRows): void {
    this.view.dispatch({ effects: setRows.of(rows) });
  }

  setWrap(on: boolean): void {
    if (this.#destroyed || this.#wrapped === on) return;
    this.#wrapped = on;
    this.view.dispatch({ effects: this.#wrap.reconfigure(on ? wrapped : []) });
  }

  setPads(pads: Float64Array | null): void {
    if (this.#destroyed) return;
    const have = this.view.state.field(paneField).pads;
    if (have === pads) return;
    if (have && pads && have.length === pads.length && have.every((value, i) => value === pads[i])) return;
    this.view.dispatch({ effects: setPads.of(pads) });
  }

  rowExtents(count: number): RowExtents {
    return rowExtents(this.view, count);
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

  /** A block of the diff back to its old lines (`lineChange`), as an edit Undo takes back;
      the caret goes back in. Text typed since the diff: nothing, the next diff is on its way. */
  revert(from: number, now: readonly string[], then: readonly string[]): void {
    const change = lineChange(this.view.state.doc, from, now, then);
    if (change) this.view.dispatch({ changes: change, userEvent: "revert" });
    this.view.focus();
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
