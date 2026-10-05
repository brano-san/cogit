import { defaultKeymap, history, historyKeymap, redo, undo } from "@codemirror/commands";
import { syntaxHighlighting } from "@codemirror/language";
import { MergeView } from "@codemirror/merge";
import { EditorState, StateEffect, Text, type Extension } from "@codemirror/state";
import { EditorView, keymap, lineNumbers } from "@codemirror/view";
import { classHighlighter } from "@lezer/highlight";
import { languageExtension, styleNonce, themeSpec } from "$lib/solver-editor";

/** The solver's editor look, with the merge view's own marks in the diff tokens. The left
    side is read-only and looks it: dimmed gutter, no caret, a label above it. */
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
});

const common: Extension = [styleNonce(), theme, lineNumbers(), syntaxHighlighting(classHighlighter)];

export interface DiffEditorEvents {
  /** Any edit, undo or redo: the page compares against the saved text itself. */
  changed: () => void;
  save: () => void;
}

/** A side-by-side editor: the base read-only on the left, the working file on the right,
    re-diffed as it is typed (the merge view's own diff, bounded so a large file stays quick). */
export class DiffEditor {
  #view: MergeView;
  #saved: Text;

  constructor(parent: HTMLElement, base: string, text: string, events: DiffEditorEvents) {
    const saveKey = keymap.of([
      {
        key: "Mod-s",
        preventDefault: true,
        run: () => {
          events.save();
          return true;
        },
      },
    ]);
    this.#view = new MergeView({
      parent,
      a: {
        doc: base,
        extensions: [common, EditorState.readOnly.of(true), EditorView.editable.of(false)],
      },
      b: {
        doc: text,
        extensions: [
          common,
          history(),
          saveKey,
          keymap.of([...historyKeymap, ...defaultKeymap]),
          EditorView.contentAttributes.of({ "aria-label": "Working tree file, editable" }),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) events.changed();
          }),
        ],
      },
      highlightChanges: true,
      gutter: true,
      diffConfig: { scanLimit: 5000, timeout: 200 },
    });
    this.#saved = this.#view.b.state.doc;
  }

  text(): string {
    return this.#view.b.state.doc.toString();
  }

  get dirty(): boolean {
    return !this.#view.b.state.doc.eq(this.#saved);
  }

  /** What is on disk now is what the editor holds. */
  markSaved(): void {
    this.#saved = this.#view.b.state.doc;
  }

  /** Reload: the file on disk replaces the editor's text, as one undoable step. */
  replace(text: string): void {
    const doc = this.#view.b.state.doc;
    this.#view.b.dispatch({ changes: { from: 0, to: doc.length, insert: text } });
    this.markSaved();
  }

  undo(): void {
    undo(this.#view.b);
  }

  redo(): void {
    redo(this.#view.b);
  }

  focus(): void {
    this.#view.b.focus();
  }

  get focused(): boolean {
    return this.#view.b.hasFocus;
  }

  async useLanguageOf(path: string): Promise<void> {
    const grammar = await languageExtension(path);
    if (!grammar) return;
    for (const side of [this.#view.a, this.#view.b]) {
      side.dispatch({ effects: StateEffect.appendConfig.of(grammar) });
    }
  }

  destroy(): void {
    this.#view.destroy();
  }
}
