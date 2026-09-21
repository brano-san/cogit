<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    label: string;
    /** Right of the label in dimmer type, for a shortcut. */
    hint?: string;
    /** Below rather than above; for anything sitting at the top of the window. */
    below?: boolean;
    children: Snippet;
  }

  let { label, hint, below = false, children }: Props = $props();

  let shown = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  /** A delay, or every pass of the mouse across a toolbar flashes a row of bubbles. */
  function enter() {
    clearTimeout(timer);
    timer = setTimeout(() => (shown = true), 400);
  }

  function leave() {
    clearTimeout(timer);
    shown = false;
  }

  $effect(() => () => clearTimeout(timer));
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<span
  class="wrap"
  onmouseenter={enter}
  onmouseleave={leave}
  onfocusin={() => (shown = true)}
  onfocusout={leave}
>
  {@render children()}
  {#if shown}
    <span class="bubble" class:below role="tooltip">
      {label}{#if hint}<span class="hint">{hint}</span>{/if}
    </span>
  {/if}
</span>

<style>
  .wrap {
    position: relative;
    display: inline-flex;
  }

  .bubble {
    position: absolute;
    left: 50%;
    bottom: calc(100% + 6px);
    z-index: 60;
    display: flex;
    gap: var(--sp-3);
    align-items: baseline;
    padding: var(--sp-2, 3px) var(--sp-3);
    transform: translateX(-50%);
    background: var(--surface-raised);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    box-shadow: 0 2px 8px rgb(0 0 0 / 35%);
    font-size: var(--fs-header);
    white-space: nowrap;
    pointer-events: none;
  }

  .bubble.below {
    top: calc(100% + 6px);
    bottom: auto;
  }

  .hint {
    color: var(--text-secondary);
    font-family: var(--font-mono);
  }
</style>
