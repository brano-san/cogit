<script lang="ts">

  /** The app's own checkbox: the native one is a white system square on a dark panel.
      The real input stays underneath for Space, Tab and screen readers. */
  interface Props {
    checked?: boolean;
    label?: string;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
  }

  let { checked = $bindable(false), label, disabled = false, onchange }: Props = $props();
</script>

<label class="checkbox" class:disabled>
  <input
    class="native"
    type="checkbox"
    bind:checked
    {disabled}
    onchange={() => onchange?.(checked)}
  />
  <span class="box" aria-hidden="true">
    <svg viewBox="0 0 10 10"><path d="M2 5.3 4.2 7.5 8 2.8" /></svg>
  </span>
  {#if label}<span>{label}</span>{/if}
</label>

<style>
  .checkbox {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: var(--sp-3);
    cursor: default;
  }

  .checkbox.disabled {
    color: var(--text-secondary);
  }

  .native {
    position: absolute;
    left: 0;
    width: 14px;
    height: 14px;
    margin: 0;
    opacity: 0;
    pointer-events: none;
  }

  .box {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 14px;
    width: 14px;
    height: 14px;
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

  .native:checked + .box {
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
  }

  .box svg {
    width: 10px;
    height: 10px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
