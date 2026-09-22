import type { CheckState } from "$lib/ref-nodes";

export interface BoxFace {
  checked: boolean;
  indeterminate: boolean;
}

export interface TriStateParams {
  state: CheckState;
  /** Applies the click to the model and returns the state the box has now. */
  toggle: () => CheckState;
}

type BoxElement = BoxFace & Pick<EventTarget, "addEventListener" | "removeEventListener">;

export function paint(box: BoxFace, state: CheckState): void {
  box.checked = state === "on";
  box.indeterminate = state === "mixed";
}

export function faceOf(box: BoxFace): CheckState {
  if (box.indeterminate) return "mixed";
  return box.checked ? "on" : "off";
}

/** A checkbox that shows a computed state (doc/12-risks.md, R-158, R-177). The click is
    never cancelled: the browser puts a cancelled click's old tick back after the microtask
    in which Svelte rendered the new one. `change` paints the toggle's result instead. */
export function triState(element: BoxElement, params: TriStateParams) {
  let current = params;
  paint(element, current.state);

  const click = (event: Event) => event.stopPropagation();
  const change = () => paint(element, current.toggle());
  element.addEventListener("click", click);
  element.addEventListener("change", change);

  return {
    update(next: TriStateParams) {
      current = next;
      paint(element, next.state);
    },
    destroy() {
      element.removeEventListener("click", click);
      element.removeEventListener("change", change);
    },
  };
}
