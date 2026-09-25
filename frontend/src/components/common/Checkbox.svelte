<script lang="ts">
  import type { Snippet } from "svelte";
  import { triState, type TriStateParams } from "$lib/tri-state-box";

  /** The app's own checkbox: the native one is a white system square on a dark panel.
      The real input stays underneath for Space, Tab and screen readers. */
  interface Props {
    checked?: boolean;
    label?: string;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
    /** A box that shows a computed state, `mixed` included (R-158): it paints what the
        toggle returns instead of flipping itself. */
    tri?: TriStateParams;
    title?: string;
    ariaLabel?: string;
    /** Takes the rest of a row, so text inside it can shrink and be cut. */
    wide?: boolean;
    /** Label content richer than `label`; the whole of it ticks the box. */
    children?: Snippet;
  }

  let {
    checked = $bindable(false),
    label,
    disabled = false,
    onchange,
    tri,
    title,
    ariaLabel,
    wide = false,
    children,
  }: Props = $props();
</script>

<label class="checkbox" class:disabled class:wide {title}>
  {#if tri}
    <input class="native" type="checkbox" {disabled} aria-label={ariaLabel} use:triState={tri} />
  {:else}
    <input
      class="native"
      type="checkbox"
      bind:checked
      {disabled}
      aria-label={ariaLabel}
      onchange={() => onchange?.(checked)}
    />
  {/if}
  <span class="box" aria-hidden="true"></span>
  {#if children}{@render children()}{:else if label}<span>{label}</span>{/if}
</label>

<style>
  /* Top-aligned, the box centred on the first line: a label that wraps keeps its box
     beside its first line. */
  .checkbox {
    --checkbox-box: var(--checkbox-size);
    position: relative;
    display: inline-flex;
    align-items: flex-start;
    gap: var(--sp-3);
    cursor: default;
  }

  .checkbox.wide {
    flex: 1 1 auto;
    min-width: 0;
  }

  .checkbox.disabled {
    color: var(--text-secondary);
  }

  .native {
    position: absolute;
    left: 0;
    width: var(--checkbox-box);
    height: var(--checkbox-box);
    margin: 0;
    opacity: 0;
    pointer-events: none;
  }

  .box {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 var(--checkbox-box);
    width: var(--checkbox-box);
    height: var(--checkbox-box);
    margin-top: max(calc((1lh - var(--checkbox-box)) / 2), 0px);
    background: var(--surface-input);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    color: transparent;
    transition:
      background var(--t-fast) var(--ease-out),
      border-color var(--t-fast) var(--ease-out);
  }

  .checkbox:hover .native:not(:disabled) + .box {
    border-color: var(--state-focus-ring);
  }

  .native:checked + .box,
  .native:indeterminate + .box {
    background: var(--status-ref);
    border-color: var(--status-ref);
    color: var(--surface-base);
  }

  .native:focus-visible + .box {
    outline: 1px solid var(--state-focus-ring);
    outline-offset: 1px;
  }

  .native:disabled + .box {
    opacity: 0.5;
    transition: none;
  }

  /* A mask, not an inline <svg>: the Branches tree draws a box on every one of hundreds of
     rows, and an SVG in each cost its first screen 6 ms on the large set. */
  .box::after {
    content: "";
    width: 10px;
    height: 10px;
    background: currentColor;
    mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10' fill='none' stroke='black' stroke-width='1.6' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='M2 5.3 4.2 7.5 8 2.8'/%3E%3C/svg%3E")
      center / contain no-repeat;
  }

  .native:indeterminate + .box::after {
    mask-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10' fill='none' stroke='black' stroke-width='1.6' stroke-linecap='round'%3E%3Cpath d='M2.5 5h5'/%3E%3C/svg%3E");
  }
</style>
