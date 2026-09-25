<script lang="ts">
  import type { Snippet } from "svelte";

  /** The app's own radio button, drawn like `Checkbox`: the native one is a white system
      circle on a dark panel. The real input stays underneath for the arrows and Tab. */
  interface Props {
    checked: boolean;
    /** Radios of one choice share it, so the arrows move between them. */
    name?: string;
    label?: string;
    disabled?: boolean;
    onchange: () => void;
    children?: Snippet;
  }

  let { checked, name, label, disabled = false, onchange, children }: Props = $props();
</script>

<label class="radio" class:disabled>
  <input class="native" type="radio" {name} {checked} {disabled} onchange={() => onchange()} />
  <span class="dot" aria-hidden="true"></span>
  {#if children}{@render children()}{:else if label}<span>{label}</span>{/if}
</label>

<style>
  .radio {
    --radio-box: var(--checkbox-size);
    position: relative;
    display: inline-flex;
    align-items: flex-start;
    gap: var(--sp-3);
    cursor: default;
  }

  .radio.disabled {
    color: var(--text-secondary);
  }

  .native {
    position: absolute;
    left: 0;
    width: var(--radio-box);
    height: var(--radio-box);
    margin: 0;
    opacity: 0;
    pointer-events: none;
  }

  .dot {
    flex: 0 0 var(--radio-box);
    width: var(--radio-box);
    height: var(--radio-box);
    margin-top: max(calc((1lh - var(--radio-box)) / 2), 0px);
    background: var(--surface-input);
    border: 1px solid var(--field-border);
    border-radius: 50%;
    transition: border-color var(--t-fast) var(--ease-out);
  }

  .radio:hover .native:not(:disabled) + .dot {
    border-color: var(--state-focus-ring);
  }

  .native:checked + .dot {
    border-color: var(--status-ref);
    background: radial-gradient(circle, var(--status-ref) 0 3px, var(--surface-input) 3.5px);
  }

  .native:focus-visible + .dot {
    outline: 1px solid var(--state-focus-ring);
    outline-offset: 1px;
  }

  .native:disabled + .dot {
    opacity: 0.5;
    transition: none;
  }
</style>
