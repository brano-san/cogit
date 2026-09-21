<script lang="ts">
  /** Reports movement as a fraction of its container, never pixels. */
  interface Props {
    direction: "vertical" | "horizontal";
    value: number;
    label: string;
    onchange: (deltaFraction: number) => void;
    onreset: () => void;
  }

  let { direction, value, label, onchange, onreset }: Props = $props();

  let element: HTMLDivElement;
  let dragging = $state(false);

  const KEYBOARD_STEP = 0.02;

  function containerExtent(): number {
    const parent = element.parentElement;
    if (!parent) return 0;
    return direction === "vertical" ? parent.clientWidth : parent.clientHeight;
  }

  function onpointerdown(event: PointerEvent) {
    element.setPointerCapture(event.pointerId);
    dragging = true;
    event.preventDefault();
  }

  function onpointermove(event: PointerEvent) {
    if (!dragging) return;
    const extent = containerExtent();
    if (extent <= 0) return;
    const delta = direction === "vertical" ? event.movementX : event.movementY;
    onchange(delta / extent);
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
  /* A visible rail, not a gap: the track is darker than either panel and carries a grip
     in the middle. `::before` widens the grab zone past the rail without moving anything,
     so the divider stays thin and is still easy to catch. */
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

  .splitter::after {
    content: "";
    position: absolute;
    top: 50%;
    left: 50%;
    translate: -50% -50%;
    background-image: radial-gradient(var(--splitter-grip) 40%, transparent 45%);
    background-size: 2px 4px;
    pointer-events: none;
  }

  .splitter.vertical {
    width: var(--w-splitter);
    cursor: col-resize;
    border-inline: 1px solid var(--divider);
  }

  .splitter.vertical::before {
    inset-block: 0;
    inset-inline: -3px;
  }

  .splitter.vertical::after {
    width: 2px;
    height: 24px;
  }

  .splitter.horizontal {
    height: var(--w-splitter);
    cursor: row-resize;
    border-block: 1px solid var(--divider);
  }

  .splitter.horizontal::before {
    inset-inline: 0;
    inset-block: -3px;
  }

  .splitter.horizontal::after {
    width: 24px;
    height: 2px;
    background-size: 4px 2px;
  }

  .splitter:hover,
  .splitter.dragging,
  .splitter:focus-visible {
    background: var(--splitter-active);
  }

  .splitter:hover::after,
  .splitter.dragging::after,
  .splitter:focus-visible::after {
    background-image: radial-gradient(var(--c-bg-window) 40%, transparent 45%);
  }
</style>
