import type { ActionReturn } from "svelte/action";
import { dropOn, moveDrag, pressDrag, type PointerDrag } from "$lib/drop-target";

export interface PointerDragOptions {
  /** What a press drags; `null` leaves it a plain press. Default: the `data-drag` of the
      nearest marked row inside the node. */
  sourceAt?: (event: PointerEvent) => string | null;
  /** The row under a point in client coordinates, already known to be inside the node.
      Default: the `data-drop` of the nearest marked row there. */
  targetAt?: (x: number, y: number) => string | null;
  /** The row a drag hovers, so it can show before the release; `null` when it leaves. */
  onover?: (target: string | null) => void;
  /** `x` and `y` are where the pointer was released, for a menu to open there. */
  ondrop: (source: string, target: string, x: number, y: number) => void;
}

/** A field or a list box keeps its own presses: text is selected there, not dragged. */
const KEEPS_PRESS = "input, textarea, select, [contenteditable]";

function marked(element: Element | null, attribute: string, node: HTMLElement): string | null {
  const row = element?.closest(`[${attribute}]`);
  return row && node.contains(row) ? row.getAttribute(attribute) : null;
}

/** The click a release makes after a drag is not a click on the row it lands on. */
function swallowNextClick(): void {
  const swallow = (event: Event) => {
    event.stopPropagation();
    event.preventDefault();
  };
  window.addEventListener("click", swallow, { capture: true, once: true });
  setTimeout(() => window.removeEventListener("click", swallow, true), 0);
}

/** Drags rows of one list onto each other on pointer events rather than HTML5
    drag-and-drop, which never reaches the page in WebView2 (R-450). `Esc` cancels. */
export function pointerDrag(node: HTMLElement, options: PointerDragOptions | null): ActionReturn<PointerDragOptions | null> {
  let current = options;
  let drag: PointerDrag | null = null;
  let pointer = -1;
  let over: string | null = null;
  /** `Esc` ended the drag; the button is still down, and its release must not click. */
  let cancelled = false;

  const sourceAt = (event: PointerEvent) =>
    current?.sourceAt ? current.sourceAt(event) : marked(event.target as Element | null, "data-drag", node);

  function targetAt(x: number, y: number): string | null {
    const hit = document.elementFromPoint(x, y);
    if (!hit || !node.contains(hit)) return null;
    return current?.targetAt ? current.targetAt(x, y) : marked(hit, "data-drop", node);
  }

  function hover(target: string | null) {
    if (target === over) return;
    over = target;
    current?.onover?.(target);
  }

  function stop() {
    window.removeEventListener("pointermove", onmove, true);
    window.removeEventListener("pointerup", onup, true);
    window.removeEventListener("pointercancel", stop, true);
    window.removeEventListener("keydown", onkey, true);
    document.documentElement.classList.remove("pointer-dragging");
    if (node.hasPointerCapture(pointer)) node.releasePointerCapture(pointer);
    hover(null);
    drag = null;
    cancelled = false;
  }

  function onmove(event: PointerEvent) {
    if (!drag || cancelled || event.pointerId !== pointer) return;
    const started = !drag.moving;
    drag = moveDrag(drag, event.clientX, event.clientY);
    if (!drag.moving) return;
    if (started) {
      // Captured only once it is a drag: a captured click lands on the node, not the row.
      node.setPointerCapture(pointer);
      document.documentElement.classList.add("pointer-dragging");
    }
    const target = targetAt(event.clientX, event.clientY);
    hover(target === drag.source ? null : target);
  }

  function onup(event: PointerEvent) {
    if (!drag || event.pointerId !== pointer) return;
    const finished = cancelled ? null : drag;
    const target = finished?.moving ? targetAt(event.clientX, event.clientY) : null;
    const dragged = cancelled || drag.moving;
    stop();
    if (!dragged) return;
    swallowNextClick();
    const at = dropOn(finished, target);
    if (finished && at !== null) current?.ondrop(finished.source, at, event.clientX, event.clientY);
  }

  function onkey(event: KeyboardEvent) {
    if (event.key !== "Escape" || !drag?.moving || cancelled) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    cancelled = true;
    hover(null);
    document.documentElement.classList.remove("pointer-dragging");
  }

  function ondown(event: PointerEvent) {
    if (!current || drag || event.button !== 0) return;
    if ((event.target as Element | null)?.closest(KEEPS_PRESS)) return;
    const source = sourceAt(event);
    if (source === null) return;
    drag = pressDrag(source, event.clientX, event.clientY);
    pointer = event.pointerId;
    window.addEventListener("pointermove", onmove, true);
    window.addEventListener("pointerup", onup, true);
    window.addEventListener("pointercancel", stop, true);
    window.addEventListener("keydown", onkey, true);
  }

  node.addEventListener("pointerdown", ondown);
  return {
    update(next) {
      current = next;
      if (!next && drag) stop();
    },
    destroy() {
      if (drag) stop();
      node.removeEventListener("pointerdown", ondown);
    },
  };
}
