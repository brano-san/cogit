import { describe, expect, it } from "vitest";
import { sameLayout } from "./toolbar-layout";
import { canUndo, emptyStack, record, undo, type UndoStack } from "./undo-stack";

type Layout = readonly string[];

const step = (stack: UndoStack<Layout>, before: Layout, after: Layout) =>
  record(stack, before, after, sameLayout);

describe("undo stack", () => {
  it("has nothing to undo when nothing was changed", () => {
    const stack = emptyStack<Layout>();
    expect(canUndo(stack)).toBe(false);
    expect(undo(stack)).toBeNull();
  });

  it("takes the changes back one at a time, newest first", () => {
    let stack = emptyStack<Layout>();
    stack = step(stack, ["pull", "push"], ["push"]);
    stack = step(stack, ["push"], ["push", "sync"]);

    const first = undo(stack);
    expect(first?.state).toEqual(["push"]);
    const second = first && undo(first.stack);
    expect(second?.state).toEqual(["pull", "push"]);
    expect(second && canUndo(second.stack)).toBe(false);
  });

  it("does not count an edit that left the layout as it was", () => {
    const stack = step(emptyStack(), ["pull"], ["pull"]);
    expect(canUndo(stack)).toBe(false);
  });

  it("forgets the oldest steps past the limit", () => {
    let stack = emptyStack<number>();
    for (let n = 0; n < 5; n++) stack = record(stack, n, n + 1, (a, b) => a === b, 3);
    expect(stack.past).toEqual([2, 3, 4]);
  });

  it("never changes a stack it was given", () => {
    const stack = step(emptyStack(), ["pull"], ["push"]);
    step(stack, ["push"], ["sync"]);
    undo(stack);
    expect(stack.past).toEqual([["pull"]]);
  });
});
