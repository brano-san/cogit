<script lang="ts">
  import { FIELD_LABELS, FILTER_FIELDS, isSlowField, SLOW_FIELD_TIP } from "$lib/filter-fields";
  import { graphFilter } from "$stores/graph-filter.svelte";

  const on = $derived(new Set(graphFilter.fields));
</script>

<!-- Where the filter's text is looked for; saved with the settings (F-560). -->
<div class="fields" role="group" aria-label="Look for the filter text in">
  {#each FILTER_FIELDS as field (field)}
    <span class="cell">
      <button
        type="button"
        class="field"
        class:on={on.has(field)}
        class:slow={isSlowField(field)}
        aria-pressed={on.has(field)}
        title={FIELD_LABELS[field].title}
        onclick={() => graphFilter.toggle(field)}>{FIELD_LABELS[field].label}</button
      >
      {#if isSlowField(field)}
        <button type="button" class="star" aria-label={SLOW_FIELD_TIP} data-tip={SLOW_FIELD_TIP}>*</button>
      {/if}
    </span>
  {/each}
</div>

<style>
  .fields {
    display: flex;
    flex: 0 0 auto;
    flex-wrap: wrap;
    gap: var(--sp-2);
    padding: var(--sp-2) var(--sp-3);
    border-bottom: 1px solid var(--divider);
  }

  .cell {
    position: relative;
    display: inline-flex;
  }

  .field {
    height: var(--h-button-sm);
    padding: 0 var(--sp-4);
    background: none;
    color: var(--text-secondary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  .field.slow {
    padding-right: calc(var(--sp-4) + 1.2ch);
  }

  .star {
    position: absolute;
    top: 0;
    right: var(--sp-3);
    bottom: 0;
    padding: 0;
    background: none;
    border: 0;
    color: var(--text-secondary);
    font: inherit;
    cursor: help;
  }

  .field:hover {
    background: var(--state-hover);
    color: var(--text-primary);
  }

  /* Pressed stays pressed under the pointer: hover must not hide which ones are on. */
  .field.on,
  .field.on:hover {
    background: var(--state-selected);
    color: var(--status-ref);
    border-color: var(--status-ref);
  }
</style>
