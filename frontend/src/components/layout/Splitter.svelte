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
  /* The visible line is 1px; the grab zone is widened with padding and a negative
     margin so the divider stays hairline-thin but is still easy to hit. */
  .splitter {
    position: relative;
    z-index: 2;
    background: var(--divider);
    flex: 0 0 auto;
    transition: background var(--t-fast) var(--ease-out);
  }

  .splitter.vertical {
    width: 1px;
    cursor: col-resize;
    padding-inline: 2px;
    margin-inline: -2px;
    background-clip: content-box;
  }

  .splitter.horizontal {
    height: 1px;
    cursor: row-resize;
    padding-block: 2px;
    margin-block: -2px;
    background-clip: content-box;
  }

  .splitter:hover,
  .splitter.dragging,
  .splitter:focus-visible {
    background: var(--state-focus-ring);
  }
</style>
