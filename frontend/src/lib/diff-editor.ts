import { defaultKeymap, history, historyKeymap, indentLess, insertNewline, insertTab, redo, undo } from "@codemirror/commands";
import { syntaxHighlighting } from "@codemirror/language";
import { EditorState, StateEffect, type Extension, type Text } from "@codemirror/state";
import { EditorView, keymap, lineNumbers, type KeyBinding } from "@codemirror/view";
import { classHighlighter } from "@lezer/highlight";
import { languageExtension, styleNonce, themeSpec } from "$lib/solver-editor";

export interface EditEvents {
  /** Any edit, undo or redo: the page compares against the saved text itself. */
  changed: () => void;
  save: () => void;
}

/** What makes a view the working file's editor, in the Edit window and in the diff alike:
    undo, Save on Ctrl+S, a tab typed as a tab (over a selection it indents the lines), Enter a
    bare line break: the new line gets no indent nobody typed. */
export function editing(events: EditEvents, keys: readonly KeyBinding[] = []): Extension {
  return [
    history(),
    keymap.of([
      { key: "Mod-s", preventDefault: true, run: () => (events.save(), true) },
      ...keys,
      { key: "Tab", run: insertTab, shift: indentLess },
      { key: "Enter", run: insertNewline, shift: insertNewline },
      ...historyKeymap,
      ...defaultKeymap,
    ]),
    EditorView.contentAttributes.of({ "aria-label": "Working tree file, editable" }),
    EditorView.updateListener.of((update) => {
      if (update.docChanged) events.changed();
    }),
  ];
}

/** The working file alone, in the Edit window (F-722). */
export class DiffEditor {
  #view: EditorView;
  #saved: Text;

  /** `done`: Esc, the window closes through the question about unsaved edits. */
  constructor(parent: HTMLElement, text: string, events: EditEvents & { done: () => void }) {
    this.#view = new EditorView({
      parent,
      state: EditorState.create({
        doc: text,
        extensions: [
          styleNonce(),
          EditorView.theme(themeSpec),
          lineNumbers(),
          syntaxHighlighting(classHighlighter),
          editing(events, [{ key: "Escape", run: () => (events.done(), true) }]),
        ],
      }),
    });
    this.#saved = this.#view.state.doc;
  }

  text(): string {
    return this.#view.state.doc.toString();
  }

  get dirty(): boolean {
    return !this.#view.state.doc.eq(this.#saved);
  }

  /** What is on disk now is `doc`, the text the save wrote. */
  markSaved(doc: Text = this.#view.state.doc): void {
    this.#saved = doc;
  }

  get doc(): Text {
    return this.#view.state.doc;
  }

  undo(): void {
    undo(this.#view);
  }

  redo(): void {
    redo(this.#view);
  }

  focus(): void {
    this.#view.focus();
  }

  get focused(): boolean {
    return this.#view.hasFocus;
  }

  async useLanguageOf(path: string): Promise<void> {
    const grammar = await languageExtension(path);
    if (grammar) this.#view.dispatch({ effects: StateEffect.appendConfig.of(grammar) });
  }

  destroy(): void {
    this.#view.destroy();
  }
}
