import { invertedEffects } from "@codemirror/commands";
import { StateEffect, StateField, type EditorState, type Extension, type TransactionSpec } from "@codemirror/state";
import type { BlockRows } from "./solver-geometry";
import { linesToText, takeLines, textToLines, type Hunk, type Span, type TakeAction } from "./solver-model";

/** A hunk's place in the Result document, in characters. It covers whole lines, newlines
    included, so a block of no lines is an empty range. */
export interface Block {
  id: number;
  from: number;
  to: number;
}

/** Replaces one block's range. A take action sets it itself: the range of what it wrote
    cannot be told from the text being replaced by mapping the change. */
export const setBlock = StateEffect.define<Block>();

function mapped(block: Block, changes: { mapPos(pos: number, assoc?: number): number }): Block {
  if (block.from === block.to) {
    return { id: block.id, from: changes.mapPos(block.from, -1), to: changes.mapPos(block.to, 1) };
  }
  // Typing at the start of the first line is in the block; typing where the next line
  // starts is not.
  return { id: block.id, from: changes.mapPos(block.from, -1), to: changes.mapPos(block.to, -1) };
}

export const blockField = StateField.define<readonly Block[]>({
  create: () => [],
  update(blocks, tr) {
    let next = tr.docChanged ? blocks.map((block) => mapped(block, tr.changes)) : blocks;
    for (const effect of tr.effects) {
      if (effect.is(setBlock)) next = next.map((block) => (block.id === effect.value.id ? effect.value : block));
    }
    return next;
  },
});

/** The field, and what Undo needs to put a block's range back with its text. */
export function blockTracker(initial: readonly Block[]): Extension {
  return [
    blockField.init(() => initial),
    invertedEffects.of((tr) => {
      const before = tr.startState.field(blockField);
      return tr.effects
        .filter((effect) => effect.is(setBlock))
        .flatMap((effect) => {
          const old = before.find((block) => block.id === (effect.value as Block).id);
          return old ? [setBlock.of(old)] : [];
        });
    }),
  ];
}

export function blocksOf(state: EditorState): readonly Block[] {
  return state.field(blockField);
}

export function blockLines(state: EditorState, block: Block): string[] {
  return textToLines(state.sliceDoc(block.from, block.to));
}

export function blockRows(state: EditorState): BlockRows[] {
  const { doc } = state;
  return blocksOf(state).map((block) => {
    const start = doc.lineAt(block.from).number - 1;
    return { start, count: block.to > block.from ? doc.lineAt(block.to - 1).number - start : 0 };
  });
}

/** The character ranges of the spans (lines) of a document of this text. */
export function initialBlocks(text: string, spans: readonly Span[]): Block[] {
  const starts = [0];
  for (let at = text.indexOf("\n"); at !== -1; at = text.indexOf("\n", at + 1)) starts.push(at + 1);
  const offset = (line: number) => starts[line] ?? text.length;
  return spans.map((span) => ({ id: span.id, from: offset(span.start), to: offset(span.start + span.count) }));
}

export function takeTransaction(state: EditorState, hunk: Hunk, action: TakeAction): TransactionSpec {
  const block = blocksOf(state).find((each) => each.id === hunk.id);
  if (!block) return {};
  const insert = linesToText(takeLines(hunk, action));
  return {
    changes: { from: block.from, to: block.to, insert },
    effects: setBlock.of({ id: block.id, from: block.from, to: block.from + insert.length }),
    userEvent: "solver.take",
  };
}
