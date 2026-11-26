<script lang="ts">
  import { readSliderNumber } from "$lib/settings-slider";

  interface Props {
    label: string;
    value: number;
    min: number;
    max: number;
    unit?: string;
    reset: number;
    onchange: (value: number) => void;
  }

  let { label, value, min, max, unit, reset, onchange }: Props = $props();
  const id = $props.id();
  let text = $state<string | null>(null);
  const invalid = $derived(text !== null && readSliderNumber(text, min, max) === null);

  $effect(() => {
    void [value, reset];
    text = null;
  });

  function commit(input: HTMLInputElement) {
    if (text === null) return;
    const next = readSliderNumber(input.value, min, max);
    if (next === null) return;
    input.value = String(next);
    text = null;
    if (next !== value) onchange(next);
  }
</script>

<div class="slider">
  <input
    type="range"
    {min}
    {max}
    {value}
    aria-label={label}
    oninput={(event) => {
      text = null;
      onchange(event.currentTarget.valueAsNumber);
    }}
  />
  <div class="number">
    <input
      type="text"
      inputmode="numeric"
      spellcheck="false"
      data-settings-number
      aria-label={`${label} value${unit ? ` (${unit})` : ""}`}
      aria-invalid={invalid}
      aria-describedby={invalid ? `${id}-error` : undefined}
      style:border-color={invalid ? "var(--status-danger)" : undefined}
      value={text ?? String(value)}
      oninput={(event) => (text = event.currentTarget.value)}
      onblur={(event) => commit(event.currentTarget)}
      onkeydown={(event) => {
        if (event.key !== "Enter" || event.isComposing) return;
        event.preventDefault();
        commit(event.currentTarget);
      }}
    />
    {#if invalid}
      <span class="error" role="alert" id={`${id}-error`}>Enter a whole number.</span>
    {/if}
  </div>
  {#if unit}<span aria-hidden="true">{unit}</span>{/if}
</div>

<style>
  .slider {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-width: 0;
  }

  input[type="range"] {
    flex: 1 1 auto;
    min-width: 0;
  }

  .number {
    position: relative;
    flex: 0 0 calc(4ch + 2 * var(--sp-3) + 2px);
  }

  .number input {
    font-variant-numeric: tabular-nums;
    text-align: right;
  }

  .error {
    position: absolute;
    top: 50%;
    right: calc(100% + var(--sp-2));
    transform: translateY(-50%);
    z-index: 1;
    padding: var(--sp-2) var(--sp-3);
    background: var(--bg-elevated);
    border: 1px solid var(--status-danger);
    border-radius: var(--r-sm);
    box-shadow: var(--shadow-popover);
    color: var(--text-primary);
    font-size: var(--fs-header);
    white-space: nowrap;
    pointer-events: none;
  }

  @container (max-width: 240px) {
    .error {
      top: auto;
      right: 0;
      bottom: calc(100% + var(--sp-1));
      transform: none;
    }
  }
</style>
