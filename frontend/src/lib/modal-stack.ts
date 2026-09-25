import { onDestroy } from "svelte";

/** The modal layers of a window, in the order they opened. Each one listens on the window,
    so each asks here whether a key is its to answer: only the top one's is, and nothing
    under a modal — a panel, the menu bar, a global shortcut — answers at all
    (doc/11 §1: modal > panel > global). */
export class ModalStack {
  #layers: symbol[] = [];

  open(): symbol {
    const layer = Symbol("modal");
    this.#layers.push(layer);
    return layer;
  }

  close(layer: symbol): void {
    const at = this.#layers.indexOf(layer);
    if (at >= 0) this.#layers.splice(at, 1);
  }

  isTop(layer: symbol): boolean {
    return this.#layers.at(-1) === layer;
  }

  get any(): boolean {
    return this.#layers.length > 0;
  }
}

export const modals = new ModalStack();

/** A layer for the component being set up, closed when the component goes. */
export function modalLayer(stack: ModalStack = modals): symbol {
  const layer = stack.open();
  onDestroy(() => stack.close(layer));
  return layer;
}

/** A menu item or its accelerator under an open modal would act behind it; only Exit,
    which asks about the modal's work itself, still runs (R-451). */
export function menuCommandRuns(id: string, stack: ModalStack): boolean {
  return !stack.any || id === "exit";
}

/** A key a modal answers: pressed inside it, or with nothing focused at all. One pressed in
    the Output window, which floats above dialogs and is not modal, is that window's. */
export function keyIsFor(panel: Element | undefined, target: EventTarget | null): boolean {
  if (!(target instanceof Node) || target === target.ownerDocument?.body) return true;
  return panel?.contains(target) ?? false;
}
