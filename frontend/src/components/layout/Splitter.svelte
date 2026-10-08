<script lang="ts">
  /** Reports movement as a fraction of its container, never pixels. */
  interface Props {
    direction: "vertical" | "horizontal";
    value: number;
    label: string;
    /** Pixels the caller's fraction is a share of, when that is not this splitter's parent:
        a share of the wrong length moved the edge faster or slower than the pointer. */
    extent?: number;
    onchange: (deltaFraction: number) => void;
    onreset: () => void;
  }

  let { direction, value, label, extent, onchange, onreset }: Props = $props();

  let element: HTMLDivElement;
  let dragging = $state(false);
  /** The pointer where the drag began, and the movement already reported from there. */
  let start = 0;
  let reported = 0;

  const KEYBOARD_STEP = 0.02;

  function containerExtent(): number {
    if (extent !== undefined) return extent;
    const parent = element.parentElement;
    if (!parent) return 0;
    return direction === "vertical" ? parent.clientWidth : parent.clientHeight;
  }

  function onpointerdown(event: PointerEvent) {
    element.setPointerCapture(event.pointerId);
    start = position(event);
    reported = 0;
    dragging = true;
    event.preventDefault();
  }

  function onpointermove(event: PointerEvent) {
    if (!dragging) return;
    const size = containerExtent();
    if (size <= 0) return;
    // From the drag start, not `movementX`: that one is in device pixels on a scaled display.
    const moved = position(event) - start;
    const delta = moved - reported;
    if (delta === 0) return;
    reported = moved;
    onchange(delta / size);
  }

  function position(event: PointerEvent): number {
    return direction === "vertical" ? event.clientX : event.clientY;
  }

  function onpointerup(event: PointerEvent) {
    if (!dragging) return;
    element.releasePointerCapture(event.pointerId);
    dragging = false;
  }

  function onkeydown(event: KeyboardEvent) {
    const decrease = direction === "vertical" ? "ArrowLeft" : "ArrowUp";
    const increase = direction === "vertical" ? "ArrowRight" : "ArrowDown";

    if (event.key === decrease) {
      onchange(-KEYBOARD_STEP);
    } else if (event.key === increase) {
      onchange(KEYBOARD_STEP);
    } else if (event.key === "Home") {
      onreset();
    } else {
      return;
    }
    event.preventDefault();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  bind:this={element}
  class="splitter {direction}"
  class:dragging
  role="separator"
  tabindex="0"
  aria-label={label}
  aria-orientation={direction === "vertical" ? "vertical" : "horizontal"}
  aria-valuenow={Math.round(value * 100)}
  aria-valuemin={12}
  aria-valuemax={88}
  {onpointerdown}
  {onpointermove}
  {onpointerup}
  {onkeydown}
  ondblclick={onreset}
></div>

<style>
  /* A thin line that lights up under the pointer. `::before` widens the grab zone past
     the line without moving anything, so the divider stays hairline and is still easy
     to catch: 1px of rail inside 7px of target. */
  .splitter {
    position: relative;
    z-index: 2;
    flex: 0 0 auto;
    background: var(--splitter-track);
    transition: background var(--t-fast) var(--ease-out);
  }

  .splitter::before {
    content: "";
    position: absolute;
    z-index: 1;
  }

  .splitter.vertical {
    width: var(--w-splitter);
    cursor: col-resize;
  }

  .splitter.vertical::before {
    inset-block: 0;
    inset-inline: -3px;
  }

  .splitter.horizontal {
    height: var(--w-splitter);
    cursor: row-resize;
  }

  .splitter.horizontal::before {
    inset-inline: 0;
    inset-block: -3px;
  }

  .splitter:hover,
  .splitter.dragging,
  .splitter:focus-visible {
    background: var(--splitter-active);
  }

</style>
