import { defaultKeymap, history, historyKeymap, undoDepth } from "@codemirror/commands";
import { LanguageDescription, syntaxHighlighting } from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import {
  Compartment,
  EditorState,
  Facet,
  StateEffect,
  StateField,
  type Extension,
  type Range,
} from "@codemirror/state";
import {
  Decoration,
  EditorView,
  GutterMarker,
  WidgetType,
  gutter,
  keymap,
  lineNumbers,
  type DecorationSet,
} from "@codemirror/view";
import { classHighlighter } from "@lezer/highlight";
import {
  LINE_HEIGHT,
  alignedPads,
  contentHeight,
  edgesOfBlocks,
  mapScroll,
  scrollToBlock,
  type BlockRows,
  type Edge,
  type PaneRows,
} from "./solver-geometry";
import {
  blockField,
  blockLines,
  blockRows,
  blockTracker,
  blocksOf,
  gitLines,
  initialBlocks,
  takeTransaction,
} from "./solver-blocks";
import {
  isUnresolved,
  paneTone,
  visibleActions,
  type Hunk,
  type SolverDocs,
  type Span,
  type TakeAction,
  type Tone,
} from "./solver-model";

export type PaneName = "ours" | "result" | "theirs";

class Pad extends WidgetType {
  constructor(
    readonly lines: number,
    readonly kind: "plain" | "del" | "add",
    readonly unresolved: boolean,
  ) {
    super();
  }

  override eq(other: Pad): boolean {
    return other.lines === this.lines && other.kind === this.kind && other.unresolved === this.unresolved;
  }

  override toDOM(): HTMLElement {
    const pad = document.createElement("div");
    pad.className = `sv-pad sv-pad-${this.kind}`;
    pad.style.height = `${this.lines * LINE_HEIGHT}px`;
    pad.setAttribute("aria-hidden", "true");
    return pad;
  }

  override get estimatedHeight(): number {
    return this.lines * LINE_HEIGHT;
  }

  override ignoreEvent(): boolean {
    return true;
  }
}

class Bar extends GutterMarker {
  override elementClass = "sv-bar-on";
}
const BAR = new Bar();

/** The page sets `user-select: none` on the body; WebKitGTK carries it into the contenteditable
    editors, where the text of all three panes then does not paint (gutters, outside the
    editable, still do). Selecting and copying is wanted here, so the editor opts back in. */
export const themeSpec = {
  "&": {
    height: "100%",
    backgroundColor: "var(--bg-editor)",
    color: "var(--fg-primary)",
    fontSize: "var(--fs-code)",
  },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": {
    fontFamily: "var(--font-mono)",
    lineHeight: `${LINE_HEIGHT}px`,
    overscrollBehavior: "contain",
  },
  ".cm-content": { padding: "0", caretColor: "var(--fg-primary)", userSelect: "text", WebkitUserSelect: "text" },
  ".cm-line": { height: `${LINE_HEIGHT}px`, lineHeight: `${LINE_HEIGHT}px`, padding: "0 6px" },
  ".cm-gutters": {
    backgroundColor: "var(--bg-editor)",
    color: "var(--diff-line-number)",
    border: "none",
    fontSize: "10px",
  },
  ".cm-lineNumbers .cm-gutterElement": { minWidth: "40px", padding: "0 6px 0 4px", textAlign: "right" },
  ".cm-activeLine": { backgroundColor: "transparent" },
  ".cm-selectionBackground": { backgroundColor: "var(--bg-selected-inactive)" },
  "&.cm-focused .cm-selectionBackground": { backgroundColor: "var(--bg-selected)" },
  ".cm-cursor": { borderLeftColor: "var(--fg-primary)" },
  ".sv-changed": { backgroundColor: "var(--diff-changed-line)" },
  ".sv-add": { backgroundColor: "var(--diff-add-line)" },
  ".sv-del": { backgroundColor: "var(--diff-del-line)" },
  ".sv-current": { boxShadow: "inset 2px 0 0 var(--selected-bar)" },
  ".sv-pad": {
    backgroundColor: "var(--diff-filler-bg)",
    backgroundImage:
      "linear-gradient(45deg, transparent calc(50% - 0.5px), var(--diff-filler-hatch) calc(50% - 0.5px), var(--diff-filler-hatch) calc(50% + 0.5px), transparent calc(50% + 0.5px))",
    backgroundSize: "6px 6px",
    userSelect: "none",
  },
  ".sv-pad-del": { backgroundColor: "var(--diff-del-line)" },
  ".sv-pad-add": { backgroundColor: "var(--diff-add-line)" },
  ".sv-bar": { width: "4px" },
  ".sv-bar .cm-gutterElement": { padding: "0", width: "4px" },
  ".sv-bar-on": { backgroundColor: "var(--status-danger)" },
};

const theme = EditorView.theme(themeSpec);

const hunksFacet = Facet.define<readonly Hunk[], readonly Hunk[]>({ combine: (all) => all[0] ?? [] });

/** Which hunks of the Result are undecided conflicts, by the text they hold now. */
export const unresolvedFacet = Facet.define<ReadonlySet<number>, ReadonlySet<number>>({
  combine: (all) => all[0] ?? new Set(),
});

function unresolvedIds(state: EditorState): ReadonlySet<number> {
  const hunks = new Map(state.facet(hunksFacet).map((hunk) => [hunk.id, hunk]));
  const ids = new Set<number>();
  for (const block of blocksOf(state)) {
    const hunk = hunks.get(block.id);
    if (hunk?.kind === "conflict" && isUnresolved(hunk, blockLines(state, block))) ids.add(block.id);
  }
  return ids;
}

/** The hunk the actions apply to, shown in every pane. */
export const setCurrent = StateEffect.define<number | null>();
const currentField = StateField.define<number | null>({
  create: () => null,
  update(value, tr) {
    for (const effect of tr.effects) if (effect.is(setCurrent)) return effect.value as number | null;
    return value;
  },
});

/** How tall the other panes shown are, per hunk, so the Result pads itself to them. */
export interface Others {
  rows: number[];
  aligned: boolean;
}
export const setOthers = StateEffect.define<Others>();
const othersField = StateField.define<Others>({
  create: () => ({ rows: [], aligned: false }),
  update(value, tr) {
    for (const effect of tr.effects) if (effect.is(setOthers)) return effect.value as Others;
    return value;
  },
});

function lineStart(state: EditorState, line: number): number {
  return line < state.doc.lines ? state.doc.line(line + 1).from : state.doc.length;
}

function resultDecorations(state: EditorState): DecorationSet {
  const blocks = blocksOf(state);
  const hunks = new Map(state.facet(hunksFacet).map((hunk) => [hunk.id, hunk]));
  const unresolved = state.facet(unresolvedFacet);
  const current = state.field(currentField);
  const others = state.field(othersField);
  const rows = blockRows(state);
  const ranges: Range<Decoration>[] = [];
  blocks.forEach((block, at) => {
    if (!hunks.has(block.id)) return;
    const { start, count } = rows[at] ?? { start: 0, count: 0 };
    const open = unresolved.has(block.id);
    const cls = `sv-changed${current === block.id ? " sv-current" : ""}`;
    for (let line = start; line < start + count; line++) {
      ranges.push(Decoration.line({ class: cls }).range(lineStart(state, line)));
    }
    const pad = others.aligned ? Math.max((others.rows[at] ?? 0) - count, 0) : 0;
    if (pad > 0) {
      const at = start + count < state.doc.lines ? lineStart(state, start + count) : state.doc.length;
      ranges.push(
        Decoration.widget({ widget: new Pad(pad, "plain", open), block: true, side: -1 }).range(at),
      );
    }
  });
  return Decoration.set(ranges, true);
}

const resultBar = gutter({
  class: "sv-bar",
  lineMarker(view, line) {
    const state = view.state;
    const unresolved = state.facet(unresolvedFacet);
    if (unresolved.size === 0) return null;
    for (const block of blocksOf(state)) {
      if (unresolved.has(block.id) && line.from >= block.from && line.from < Math.max(block.to, block.from + 1)) {
        return BAR;
      }
    }
    return null;
  },
  widgetMarker: (_view, widget) => (widget instanceof Pad && widget.unresolved ? BAR : null),
  lineMarkerChange: (update) =>
    update.docChanged || update.transactions.some((tr) => tr.effects.length > 0),
  initialSpacer: () => BAR,
});

function resultExtensions(docs: SolverDocs): Extension {
  return [
    hunksFacet.of(docs.hunks),
    blockTracker(initialBlocks(docs.resultText, docs.spans)),
    currentField,
    othersField,
    unresolvedFacet.compute([blockField, "doc"], unresolvedIds),
    EditorView.decorations.compute([blockField, "doc", currentField, othersField, unresolvedFacet], resultDecorations),
    resultBar,
    history(),
    keymap.of([...historyKeymap, ...defaultKeymap]),
    EditorView.contentAttributes.of({ "aria-label": "Result" }),
  ];
}

/** What Ours and Theirs show around each hunk: its tone, and the filler that levels it. */
export interface PaneSpec {
  rows: BlockRows[];
  ids: number[];
  tones: (Tone | null)[];
  pads: number[];
  current: number | null;
}

const EMPTY_SPEC: PaneSpec = { rows: [], ids: [], tones: [], pads: [], current: null };
export const setPaneSpec = StateEffect.define<PaneSpec>();
const specField = StateField.define<PaneSpec>({
  create: () => EMPTY_SPEC,
  update(value, tr) {
    for (const effect of tr.effects) if (effect.is(setPaneSpec)) return effect.value as PaneSpec;
    return value;
  },
});

function paneDecorations(state: EditorState): DecorationSet {
  const spec = state.field(specField);
  const ranges: Range<Decoration>[] = [];
  spec.rows.forEach((row, at) => {
    const tone = spec.tones[at];
    const cls = `${tone ? `sv-${tone}` : ""}${spec.ids[at] === spec.current ? " sv-current" : ""}`.trim();
    if (cls) {
      for (let line = row.start; line < row.start + row.count; line++) {
        ranges.push(Decoration.line({ class: cls }).range(lineStart(state, line)));
      }
    }
    const pad = spec.pads[at] ?? 0;
    if (pad > 0) {
      const kind = row.count === 0 && tone === "del" ? "del" : "plain";
      const place = row.start + row.count < state.doc.lines ? lineStart(state, row.start + row.count) : state.doc.length;
      ranges.push(Decoration.widget({ widget: new Pad(pad, kind, false), block: true, side: -1 }).range(place));
    }
  });
  return Decoration.set(ranges, true);
}

function sideExtensions(label: string): Extension {
  return [
    specField,
    EditorView.decorations.compute([specField, "doc"], paneDecorations),
    EditorState.readOnly.of(true),
    keymap.of(defaultKeymap),
    EditorView.contentAttributes.of({ "aria-label": label }),
  ];
}

const language = new Compartment();

/** The grammar for a path, parsed in its own chunk; `null` for a file nobody knows. */
export async function languageExtension(path: string): Promise<Extension | null> {
  const name = path.slice(path.lastIndexOf("/") + 1);
  const found = LanguageDescription.matchFilename(languages, name);
  if (!found) return null;
  try {
    return await found.load();
  } catch (err) {
    console.error("the grammar did not load", err);
    return null;
  }
}

const common: Extension = [gitLines, theme, lineNumbers(), syntaxHighlighting(classHighlighter)];

export interface EditorOptions {
  aligned: boolean;
  baseChanges: boolean;
  /** Panes that are on screen; hidden ones neither pad nor follow. */
  shown: { ours: boolean; theirs: boolean };
  /** Result below: Ours and Theirs line up with each other, the Result is on its own. */
  resultBelow: boolean;
}

export interface Snapshot {
  /** Bumped by every change of anything the bands and the toolbar read. */
  version: number;
  rows: PaneRows[];
  unresolved: number[];
  /** Per hunk: what a Take button would change. */
  actions: { ours: boolean; theirs: boolean }[];
  dirty: boolean;
  currentId: number | null;
}

export interface Geometry {
  ours: Edge[];
  result: Edge[];
  theirs: Edge[];
  content: Record<PaneName, number>;
  scroll: Record<PaneName, number>;
  viewport: Record<PaneName, number>;
}

interface Hosts {
  ours: HTMLElement;
  result: HTMLElement;
  theirs: HTMLElement;
}

/** The three editors of the Conflict Solver and everything that keeps them one view of one
    file: padding, scroll, the hunk that is current. Svelte reads `snapshot` and `geometry`. */
export class SolverEditors {
  readonly views: Record<PaneName, EditorView>;
  readonly #docs: SolverDocs;
  #options: EditorOptions;
  #current: number | null = null;
  #version = 0;
  #snapshot: Snapshot;
  #pending = false;
  #expected: Record<PaneName, number | null> = { ours: null, result: null, theirs: null };
  readonly #onchange: () => void;
  #destroyed = false;

  constructor(hosts: Hosts, docs: SolverDocs, options: EditorOptions, onchange: () => void) {
    this.#docs = docs;
    this.#options = options;
    this.#onchange = onchange;
    const watch = (name: PaneName) =>
      EditorView.updateListener.of((update) => {
        if (name === "result") {
          if (update.selectionSet && !update.docChanged) this.#followSelection(name);
          if (update.docChanged || update.transactions.some((tr) => tr.effects.length > 0)) this.#schedule();
        } else if (update.selectionSet) {
          this.#followSelection(name);
        }
      });
    const make = (name: PaneName, parent: HTMLElement, doc: string, extensions: Extension) =>
      new EditorView({
        parent,
        state: EditorState.create({ doc, extensions: [common, language.of([]), extensions, watch(name)] }),
      });
    this.views = {
      ours: make("ours", hosts.ours, docs.oursText, sideExtensions("Ours")),
      theirs: make("theirs", hosts.theirs, docs.theirsText, sideExtensions("Theirs")),
      result: make("result", hosts.result, docs.resultText, resultExtensions(docs)),
    };
    this.#snapshot = this.#compute();
    for (const name of ["ours", "result", "theirs"] as const) {
      this.views[name].scrollDOM.addEventListener("scroll", () => this.#scrolled(name));
    }
    this.#relayout();
  }

  /** The hunk a line of a pane belongs to; a hunk of no lines owns the line it sits before. */
  #hunkAtLine(name: PaneName, line: number): number | null {
    const rows =
      name === "result"
        ? blockRows(this.views.result.state)
        : name === "ours"
          ? this.#docs.oursRows
          : this.#docs.theirsRows;
    const at = rows.findIndex((row) => line >= row.start && line < row.start + Math.max(row.count, 1));
    return at === -1 ? null : (this.#docs.hunks[at]?.id ?? null);
  }

  /** The hunk under the caret becomes the current one. */
  #followSelection(name: PaneName): void {
    const view = this.views[name];
    const id = this.#hunkAtLine(name, view.state.doc.lineAt(view.state.selection.main.head).number - 1);
    if (id !== null && id !== this.#current) queueMicrotask(() => this.setCurrent(id));
  }

  /** The hunk at a point of the screen, for a right click. */
  hunkAt(name: PaneName, x: number, y: number): number | null {
    const view = this.views[name];
    const pos = view.posAtCoords({ x, y }, false);
    return this.#hunkAtLine(name, view.state.doc.lineAt(pos).number - 1);
  }

  #schedule(): void {
    if (this.#pending || this.#destroyed) return;
    this.#pending = true;
    queueMicrotask(() => {
      this.#pending = false;
      if (this.#destroyed) return;
      this.#relayout();
      this.#snapshot = this.#compute();
      this.#onchange();
    });
  }

  #rows(): PaneRows[] {
    const result = blockRows(this.views.result.state);
    return this.#docs.hunks.map((_, at) => ({
      ours: this.#docs.oursRows[at]?.count ?? 0,
      result: result[at]?.count ?? 0,
      theirs: this.#docs.theirsRows[at]?.count ?? 0,
    }));
  }

  #compute(): Snapshot {
    const state = this.views.result.state;
    const unresolved = unresolvedIds(state);
    const blocks = blocksOf(state);
    this.#version += 1;
    return {
      version: this.#version,
      rows: this.#rows(),
      unresolved: [...unresolved],
      actions: this.#docs.hunks.map((hunk, at) => {
        const block = blocks[at];
        return block ? visibleActions(hunk, blockLines(state, block)) : { ours: false, theirs: false };
      }),
      dirty: undoDepth(state) > 0,
      currentId: this.#current,
    };
  }

  /** Padding and tones, from the Result as it is now. */
  #relayout(): void {
    const { aligned, baseChanges, shown, resultBelow } = this.#options;
    const rows = this.#rows();
    const pads = alignedPads(
      rows,
      aligned
        ? { ours: shown.ours, result: !resultBelow, theirs: shown.theirs }
        : { ours: false, result: false, theirs: false },
    );
    const others = rows.map((row) =>
      Math.max(shown.ours ? row.ours : 0, shown.theirs ? row.theirs : 0),
    );
    this.#paint("ours", pads.ours, baseChanges);
    this.#paint("theirs", pads.theirs, baseChanges);
    const result = this.views.result;
    const wanted: Others = { rows: others, aligned: aligned && !resultBelow };
    const have = result.state.field(othersField);
    if (have.aligned !== wanted.aligned || have.rows.join() !== wanted.rows.join()) {
      result.dispatch({ effects: setOthers.of(wanted), annotations: [] });
    }
  }

  #paint(name: "ours" | "theirs", pads: number[], baseChanges: boolean): void {
    const view = this.views[name];
    const rows = name === "ours" ? this.#docs.oursRows : this.#docs.theirsRows;
    const spec: PaneSpec = {
      rows,
      ids: this.#docs.hunks.map((hunk) => hunk.id),
      tones: this.#docs.hunks.map((hunk) => paneTone(hunk, name, baseChanges)),
      pads,
      current: this.#current,
    };
    const have = view.state.field(specField);
    if (
      have.current === spec.current &&
      have.pads.join() === spec.pads.join() &&
      have.tones.join() === spec.tones.join() &&
      have.rows === spec.rows
    ) {
      return;
    }
    view.dispatch({ effects: setPaneSpec.of(spec) });
  }

  get snapshot(): Snapshot {
    return this.#snapshot;
  }

  get hunks(): readonly Hunk[] {
    return this.#docs.hunks;
  }

  setOptions(options: EditorOptions): void {
    this.#options = options;
    this.#relayout();
    this.#snapshot = this.#compute();
    for (const view of Object.values(this.views)) view.requestMeasure();
    this.#onchange();
  }

  setLanguage(extension: Extension | null): void {
    for (const view of Object.values(this.views)) {
      view.dispatch({ effects: language.reconfigure(extension ?? []) });
    }
  }

  get currentId(): number | null {
    return this.#current;
  }

  setCurrent(id: number | null): void {
    if (this.#current === id) return;
    this.#current = id;
    this.views.result.dispatch({ effects: setCurrent.of(id) });
    this.#relayout();
    this.#snapshot = this.#compute();
    this.#onchange();
  }

  take(id: number, action: TakeAction): void {
    const hunk = this.#docs.hunks.find((each) => each.id === id);
    if (!hunk) return;
    const result = this.views.result;
    result.dispatch(takeTransaction(result.state, hunk, action));
  }

  /** Brings a hunk into view in every pane, the Result first; the others follow by the sync. */
  goTo(id: number): void {
    const at = this.#docs.hunks.findIndex((hunk) => hunk.id === id);
    if (at === -1) return;
    this.setCurrent(id);
    const geometry = this.geometry();
    const edge = geometry.result[at];
    if (!edge) return;
    const view = this.views.result;
    const top = scrollToBlock(edge, view.scrollDOM.clientHeight, geometry.content.result);
    this.#write("result", top);
    // The write is a scroll the Result swallows as its own echo, so the others are moved here.
    this.#syncOthers("result", top);
    this.#onchange();
  }

  #resultPads(): number[] {
    const others = this.views.result.state.field(othersField);
    const rows = blockRows(this.views.result.state);
    return rows.map((row, at) => (others.aligned ? Math.max((others.rows[at] ?? 0) - row.count, 0) : 0));
  }

  geometry(): Geometry {
    const oursPads = this.views.ours.state.field(specField).pads;
    const theirsPads = this.views.theirs.state.field(specField).pads;
    const resultPads = this.#resultPads();
    const resultRows = blockRows(this.views.result.state);
    const scroller = (name: PaneName) => this.views[name].scrollDOM;
    const lines = (name: PaneName) => this.views[name].state.doc.lines;
    return {
      ours: edgesOfBlocks(this.#docs.oursRows, oursPads),
      theirs: edgesOfBlocks(this.#docs.theirsRows, theirsPads),
      result: edgesOfBlocks(resultRows, resultPads),
      content: {
        ours: contentHeight(lines("ours"), oursPads),
        theirs: contentHeight(lines("theirs"), theirsPads),
        result: contentHeight(lines("result"), resultPads),
      },
      scroll: { ours: scroller("ours").scrollTop, result: scroller("result").scrollTop, theirs: scroller("theirs").scrollTop },
      viewport: {
        ours: scroller("ours").clientHeight,
        result: scroller("result").clientHeight,
        theirs: scroller("theirs").clientHeight,
      },
    };
  }

  #scrolled(name: PaneName): void {
    const top = this.views[name].scrollDOM.scrollTop;
    const want = this.#expected[name];
    if (want !== null) {
      this.#expected[name] = null;
      if (Math.abs(top - want) <= 1) {
        this.#onchange();
        return;
      }
    }
    this.#syncOthers(name, top);
    this.#onchange();
  }

  /** Puts the other panes where `name` at `top` says they belong. */
  #syncOthers(name: PaneName, top: number): void {
    const geometry = this.geometry();
    for (const other of ["ours", "result", "theirs"] as const) {
      if (other === name || geometry.viewport[other] === 0) continue;
      this.#write(
        other,
        mapScroll(
          geometry[name],
          geometry[other],
          top,
          geometry.content[name],
          geometry.content[other],
          geometry.viewport[other],
        ),
      );
    }
  }

  #write(name: PaneName, top: number): void {
    const scroller = this.views[name].scrollDOM;
    if (Math.abs(scroller.scrollTop - top) < 1) return;
    this.#expected[name] = top;
    scroller.scrollTop = top;
  }

  /** The Result as it will be written, markers or not being the caller's business. */
  get resultText(): string {
    return this.views.result.state.doc.toString();
  }

  /** Where each hunk is in the Result as it stands, for writing the undecided ones as markers. */
  spans(): Span[] {
    const rows = blockRows(this.views.result.state);
    return blocksOf(this.views.result.state).map((block, at) => ({
      id: block.id,
      start: rows[at]?.start ?? 0,
      count: rows[at]?.count ?? 0,
    }));
  }

  focusResult(): void {
    this.views.result.focus();
  }

  destroy(): void {
    this.#destroyed = true;
    for (const view of Object.values(this.views)) view.destroy();
  }
}
