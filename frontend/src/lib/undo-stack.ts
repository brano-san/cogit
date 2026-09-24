/** Step-by-step undo of edits that are applied as they are made: each entry is the state
    before one edit, newest last. */
export interface UndoStack<T> {
  readonly past: readonly T[];
}

export const UNDO_LIMIT = 100;

export function emptyStack<T>(): UndoStack<T> {
  return { past: [] };
}

/** An edit that changed nothing is not a step: Undo on it would seem to do nothing. */
export function record<T>(
  stack: UndoStack<T>,
  before: T,
  after: T,
  same: (a: T, b: T) => boolean,
  limit = UNDO_LIMIT,
): UndoStack<T> {
  if (same(before, after)) return stack;
  return { past: [...stack.past, before].slice(-limit) };
}

export function canUndo<T>(stack: UndoStack<T>): boolean {
  return stack.past.length > 0;
}

/** The state to go back to, and the stack without it; null when there is nothing left. */
export function undo<T>(stack: UndoStack<T>): { state: T; stack: UndoStack<T> } | null {
  const state = stack.past.at(-1);
  if (state === undefined) return null;
  return { state, stack: { past: stack.past.slice(0, -1) } };
}
