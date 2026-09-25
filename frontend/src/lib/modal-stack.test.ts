import { describe, expect, it } from "vitest";
import { ModalStack, menuCommandRuns } from "./modal-stack";

// Every modal listened on the window in the order it mounted: Esc in "Save as preset"
// closed the Hooks window under it too, and Enter ran the dialog underneath.
describe("ModalStack", () => {
  it("gives the keys to the layer opened last", () => {
    const stack = new ModalStack();
    const hooks = stack.open();
    const name = stack.open();
    expect(stack.isTop(name)).toBe(true);
    expect(stack.isTop(hooks)).toBe(false);
  });

  it("gives them back to the layer under once the top one closes", () => {
    const stack = new ModalStack();
    const hooks = stack.open();
    const name = stack.open();
    stack.close(name);
    expect(stack.isTop(hooks)).toBe(true);
  });

  it("keeps the top layer when one under it closes first", () => {
    const stack = new ModalStack();
    const under = stack.open();
    const top = stack.open();
    stack.close(under);
    expect(stack.isTop(top)).toBe(true);
    expect(stack.any).toBe(true);
  });

  it("is empty once every layer has closed, whatever the order", () => {
    const stack = new ModalStack();
    const a = stack.open();
    const b = stack.open();
    stack.close(a);
    stack.close(b);
    stack.close(b);
    expect(stack.any).toBe(false);
  });
});

// Ctrl+S under Edit Author opened Stash All over it; Ctrl+W closed the repository under
// a dialog; Ctrl+, under Preferences took a new snapshot, and Cancel reverted nothing.
describe("menuCommandRuns", () => {
  it("runs every command while no modal is open", () => {
    expect(menuCommandRuns("stash", new ModalStack())).toBe(true);
  });

  it("runs nothing under an open modal but Exit", () => {
    const stack = new ModalStack();
    stack.open();
    expect(menuCommandRuns("stash", stack)).toBe(false);
    expect(menuCommandRuns("close", stack)).toBe(false);
    expect(menuCommandRuns("settings", stack)).toBe(false);
    expect(menuCommandRuns("exit", stack)).toBe(true);
  });
});
