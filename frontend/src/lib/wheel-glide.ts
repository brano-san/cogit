/**
 * The wheel over a list, glided by us instead of the webview. Chromium animates every notch
 * the same long way and eases into a clamped end: with one or two rows of overflow the list
 * crept there and felt heavy, while a long list ran free. Here each frame closes a fixed share
 * of what is left, so a short and a long list move at one speed.
 */

/** Time constant of the glide: about 63 % of the way in this many ms, done in ~4×. */
export const GLIDE_TAU_MS = 35;

/** Pixels a wheel event asks for, whatever unit it came in. */
export function wheelPixels(delta: number, mode: number, line: number, page: number): number {
  if (mode === 1) return delta * line;
  if (mode === 2) return delta * page;
  return delta;
}

/** Where a glide aims after one more notch: from the old aim while moving, kept in range. */
export function nextTarget(current: number, target: number | null, delta: number, max: number): number {
  return Math.min(Math.max((target ?? current) + delta, 0), max);
}

/** One frame's step towards the aim; snaps the last pixel so the glide ends. The element keeps
    whole device pixels, so a smaller remainder would never be crossed. */
export function glideStep(current: number, target: number, dtMs: number, tau = GLIDE_TAU_MS): number {
  const left = target - current;
  if (Math.abs(left) < 1) return target;
  return current + left * (1 - Math.exp(-Math.max(dtMs, 0) / tau));
}

export interface WheelGlide {
  onwheel: (event: WheelEvent) => void;
  /** The user dragged the thumb or a key moved the list: the glide lets go. */
  stop: () => void;
}

/** Wires the glide to one scrolling element; `line` is the list's row height. */
export function wheelGlide(element: () => HTMLElement | undefined, line: () => number): WheelGlide {
  let target: number | null = null;
  /** The glide's own position: the element rounds what it is given to device pixels. */
  let pos = 0;
  let frame = 0;
  let last = 0;

  const stop = () => {
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
    target = null;
  };

  const tick = (now: number) => {
    const el = element();
    if (!el || target === null) return stop();
    // Something else moved the list (a reveal, the scrollbar): it wins.
    if (Math.abs(el.scrollTop - pos) > 1.5) return stop();
    // The list may have shrunk under the glide.
    target = Math.min(target, Math.max(el.scrollHeight - el.clientHeight, 0));
    pos = glideStep(pos, target, now - last);
    last = now;
    el.scrollTop = pos;
    if (pos === target) return stop();
    frame = requestAnimationFrame(tick);
  };

  const onwheel = (event: WheelEvent) => {
    const el = element();
    // Zoom, sideways scroll and a list that does not scroll stay the browser's.
    if (!el || event.ctrlKey || event.shiftKey) return;
    if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) return;
    const max = el.scrollHeight - el.clientHeight;
    if (max <= 0) return;
    const delta = wheelPixels(event.deltaY, event.deltaMode, line(), el.clientHeight);
    const aim = nextTarget(el.scrollTop, target, delta, max);
    // Already at the end it is going to: let the panel around it have the wheel.
    if (target === null && aim === el.scrollTop) return;
    event.preventDefault();
    // A diagonal trackpad swipe keeps its sideways part (Files' wide columns).
    if (event.deltaX !== 0) el.scrollLeft += wheelPixels(event.deltaX, event.deltaMode, line(), el.clientWidth);
    if (target === null) pos = el.scrollTop;
    target = aim;
    if (reducedMotion()) {
      el.scrollTop = aim;
      return stop();
    }
    if (!frame) {
      last = performance.now();
      frame = requestAnimationFrame(tick);
    }
  };

  return { onwheel, stop };
}

function reducedMotion(): boolean {
  return typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
}
