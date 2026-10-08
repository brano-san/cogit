import { defaultKeymap, history, historyKeymap, indentLess, insertTab, redo, undo } from "@codemirror/commands";
import { syntaxHighlighting } from "@codemirror/language";
import { MergeView, getChunks } from "@codemirror/merge";
import { EditorSelection, EditorState, RangeSetBuilder, StateEffect, Text, type Extension } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, keymap, lineNumbers, type DecorationSet, type ViewUpdate } from "@codemirror/view";
import { classHighlighter } from "@lezer/highlight";
import { languageExtension, styleNonce, themeSpec } from "$lib/solver-editor";

/** The solver's editor look, with the merge view's own marks in the diff tokens. The left
    side is read-only and looks it: dimmed gutter, no caret. The rows that level the two
    sides are hatched, as the 1:1 diff draws them. */
const theme = EditorView.theme({
  ...themeSpec,
  ".cm-changedLine": { backgroundColor: "var(--diff-changed-line)" },
  ".cm-merge-a .cm-changedLine, .cm-deletedChunk": { backgroundColor: "var(--diff-del-line)" },
  ".cm-merge-b .cm-changedLine, .cm-insertedLine": { backgroundColor: "var(--diff-add-line)" },
  ".cm-merge-a .cm-changedText, .cm-deletedChunk .cm-deletedText": { backgroundColor: "var(--diff-del-word)" },
  ".cm-merge-b .cm-changedText": { backgroundColor: "var(--diff-add-word)" },
  ".cm-changeGutter": { width: "3px", paddingLeft: "0" },
  ".cm-merge-a .cm-changedLineGutter, .cm-deletedLineGutter": { backgroundColor: "var(--diff-del-gutter)" },
  ".cm-merge-b .cm-changedLineGutter, .cm-insertedLineGutter": { backgroundColor: "var(--diff-add-gutter)" },
  ".cm-merge-a .cm-content": { caretColor: "transparent" },
  ".cm-merge-a .cm-gutters": { opacity: "0.7" },
  ".cm-mergeSpacer": themeSpec[".sv-pad"],
  // A line replaced on both sides is "changed", as the diff paints it; a side alone stays red or green.
  ".cm-line.dv-changed": { backgroundColor: "var(--diff-changed-line)" },
  ".dv-changed .cm-changedText": { backgroundColor: "var(--diff-changed-word)" },
  // Folded lines look like the diff's fold rows, not the merge view's light bar.
  ".cm-collapsedLines": {
    padding: "0 var(--sp-4)",
    background: "var(--diff-hunk-header-bg)",
    color: "var(--diff-hunk-header-fg)",
    boxShadow: "inset 0 1px 0 var(--border), inset 0 -1px 0 var(--border)",
    fontFamily: "var(--font-ui)",
    fontSize: "var(--fs-header)",
    cursor: "pointer",
  },
  ".cm-collapsedLines:before, .cm-collapsedLines:after": { display: "none" },
});

/** Lines of a chunk that changed on both sides: the diff's "changed" colour, not red and green. */
const changedPairs = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = paired(view);
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged || getChunks(update.state) !== getChunks(update.startState))
        this.decorations = paired(update.view);
    }
  },
  { decorations: (plugin) => plugin.decorations },
);

const CHANGED = Decoration.line({ class: "dv-changed" });

function paired(view: EditorView): DecorationSet {
  const found = getChunks(view.state);
  const builder = new RangeSetBuilder<Decoration>();
  if (!found) return builder.finish();
  const doc = view.state.doc;
  for (const chunk of found.chunks) {
    if (chunk.fromA >= chunk.toA || chunk.fromB >= chunk.toB) continue;
    const [from, to] = found.side === "a" ? [chunk.fromA, chunk.toA] : [chunk.fromB, chunk.toB];
    for (let pos = from; pos < Math.min(to, doc.length + 1); ) {
      const line = doc.lineAt(pos);
      builder.add(line.from, line.from, CHANGED);
      pos = line.to + 1;
    }
  }
  return builder.finish();
}

const common: Extension = [styleNonce(), theme, lineNumbers(), syntaxHighlighting(classHighlighter), changedPairs];

export interface DiffEditorEvents {
  /** Any edit, undo or redo: the page compares against the saved text itself. */
  changed: () => void;
  save: () => void;
  /** Esc: back to the diff, through the question about unsaved edits. */
  done: () => void;
}

/** The file editable: beside its base, re-diffed as it is typed (the merge view's own diff,
    bounded so a large file stays quick), or alone (`base` null: Edit from the file menu). */
export class DiffEditor {
  #merge: MergeView | null = null;
  #view: EditorView;
  #base: EditorView | null = null;
  #saved: Text;

  /** `context`: lines kept around a change when the unchanged ones fold, as the diff
      folds them; `null` folds nothing. */
  constructor(
    parent: HTMLElement,
    base: string | null,
    text: string,
    events: DiffEditorEvents,
    context: number | null = null,
  ) {
    const editable: Extension = [
      common,
      history(),
      keymap.of([
        { key: "Mod-s", preventDefault: true, run: () => (events.save(), true) },
        { key: "Escape", run: () => (events.done(), true) },
        // A tab at the caret, as a text editor types it; over a selection it indents the lines.
        { key: "Tab", run: insertTab, shift: indentLess },
        ...historyKeymap,
        ...defaultKeymap,
      ]),
      EditorView.contentAttributes.of({ "aria-label": "Working tree file, editable" }),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) events.changed();
      }),
    ];
    if (base === null) {
      this.#view = new EditorView({ parent, state: EditorState.create({ doc: text, extensions: editable }) });
    } else {
      this.#merge = new MergeView({
        parent,
        a: { doc: base, extensions: [common, EditorState.readOnly.of(true), EditorView.editable.of(false)] },
        b: { doc: text, extensions: editable },
        highlightChanges: true,
        gutter: true,
        diffConfig: { scanLimit: 5000, timeout: 50 },
        collapseUnchanged: context === null ? undefined : { margin: Math.max(context, 1), minSize: 4 },
      });
      this.#view = this.#merge.b;
      this.#base = this.#merge.a;
    }
    this.#saved = this.#view.state.doc;
  }

  text(): string {
    return this.#view.state.doc.toString();
  }

  get dirty(): boolean {
    return !this.#view.state.doc.eq(this.#saved);
  }

  /** What is on disk now is what the editor holds. */
  markSaved(): void {
    this.#saved = this.#view.state.doc;
  }

  undo(): void {
    undo(this.#view);
  }

  redo(): void {
    redo(this.#view);
  }

  /** The caret at a line and column of the file (1-based line), scrolled into view: at
      `offset` pixels from the top when given, where the clicked row of the diff was, so
      the text does not move under the pointer. */
  focusAt(line: number | null, column = 0, offset: number | null = null): void {
    const doc = this.#view.state.doc;
    if (line !== null && line >= 1 && line <= doc.lines) {
      const at = doc.line(line);
      const pos = at.from + Math.min(column, at.length);
      const place = offset === null ? { y: "center" as const } : { y: "start" as const, yMargin: Math.max(offset, 0) };
      this.#view.dispatch({ selection: EditorSelection.cursor(pos), effects: EditorView.scrollIntoView(pos, place) });
    }
    this.#view.focus();
  }

  get focused(): boolean {
    return this.#view.hasFocus;
  }

  async useLanguageOf(path: string): Promise<void> {
    const grammar = await languageExtension(path);
    if (!grammar) return;
    for (const side of [this.#base, this.#view]) {
      side?.dispatch({ effects: StateEffect.appendConfig.of(grammar) });
    }
  }

  destroy(): void {
    if (this.#merge) this.#merge.destroy();
    else this.#view.destroy();
  }
}
