import { afterEach, describe, expect, it, vi } from "vitest";
import { pointerDrag } from "./pointer-drag";

function press(target: EventTarget, type: string, button = 0): void {
  const event = new Event(type);
  Object.defineProperty(event, "button", { value: button });
  target.dispatchEvent(event);
}

function selects(): boolean {
  const event = new Event("selectstart", { cancelable: true });
  window.dispatchEvent(event);
  return !event.defaultPrevented;
}

describe("pointerDrag", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("selects no text while a press on the list is held, drag source or not", () => {
    vi.stubGlobal("window", new EventTarget());
    const node = Object.assign(new EventTarget(), { closest: () => null }) as unknown as HTMLElement;
    pointerDrag(node, { sourceAt: () => null, ondrop: () => {} });

    expect(selects()).toBe(true);
    press(node, "pointerdown");
    expect(selects()).toBe(false);
    press(window, "pointerup");
    expect(selects()).toBe(true);
    press(node, "pointerdown", 2);
    expect(selects()).toBe(true);
  });
});
