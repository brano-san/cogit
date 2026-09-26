<script lang="ts">
  import { FIELD_LABELS, FILTER_FIELDS } from "$lib/filter-fields";
  import { graphFilter } from "$stores/graph-filter.svelte";

  const on = $derived(new Set(graphFilter.fields));
</script>

<!-- Where the filter's text is looked for; saved with the settings (F-560). -->
<div class="fields" role="group" aria-label="Look for the filter text in">
  {#each FILTER_FIELDS as field (field)}
    <button
      type="button"
      class="field"
      class:on={on.has(field)}
      aria-pressed={on.has(field)}
      title={FIELD_LABELS[field].title}
      onclick={() => graphFilter.toggle(field)}>{FIELD_LABELS[field].label}</button
    >
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
