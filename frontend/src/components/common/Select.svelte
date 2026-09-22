<script lang="ts" generics="T extends string">
  /** The application's dropdown. It is a real `<select>` underneath with its native
      chrome turned off: the keyboard behaviour, the type-ahead and the screen-reader
      support of the platform control are worth more than the last few pixels of the
      popup list, which Windows still draws itself (doc/12-risks.md, R-116). */
  interface Props {
    value: T;
    options: readonly (readonly [T, string])[];
    onchange: (value: T) => void;
    label?: string;
    disabled?: boolean;
    id?: string;
  }

  let { value, options, onchange, label, disabled = false, id }: Props = $props();
</script>

<span class="select" class:disabled>
  <select
    {id}
    {value}
    {disabled}
    aria-label={label}
    onchange={(event) => onchange(event.currentTarget.value as T)}
  >
    {#each options as [key, title] (key)}
      <option value={key}>{title}</option>
    {/each}
  </select>
  <span class="caret" aria-hidden="true">▾</span>
</span>

<style>
  .select {
    position: relative;
    display: inline-flex;
    align-items: center;
    min-width: 0;
  }

  select {
    appearance: none;
    width: 100%;
    height: var(--h-input);
    padding: 0 calc(var(--sp-5) + 8px) 0 var(--sp-4);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font: inherit;
    font-size: var(--fs-dense);
    cursor: default;
  }

  select:hover:not(:disabled) {
    border-color: var(--state-focus-ring);
  }

  select:focus-visible {
    outline: 1px solid var(--state-focus-ring);
    outline-offset: -1px;
  }

  .caret {
    position: absolute;
    right: var(--sp-3);
    color: var(--text-secondary);
    font-size: 10px;
    pointer-events: none;
  }

  .disabled select {
    color: var(--text-secondary);
    opacity: 0.6;
  }

  /* The popup list is the platform's, so its items need a readable pair of their own. */
  option {
    background: var(--surface-panel);
    color: var(--text-primary);
  }
</style>
