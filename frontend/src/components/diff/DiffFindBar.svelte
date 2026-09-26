<script lang="ts">
  import type { DiffSearch } from "$lib/diff-search.svelte";

  let {
    find,
    reveal,
  }: {
    find: DiffSearch;
    /** Scrolling belongs to the panel that owns the scroller, not to this bar. */
    reveal: (index: number) => void;
  } = $props();

  let box: HTMLInputElement | undefined = $state();

  export function focus() {
    box?.select();
  }

  /** Enter walks the hits; the input keeps the key to itself so the page does not scroll. */
  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Enter") {
      event.preventDefault();
      find.go(event.shiftKey ? -1 : 1, reveal);
    }
  }
</script>

<div class="find">
  <input
    bind:this={box}
    class:missing={find.missing}
    value={find.query}
    oninput={(event) => find.setQuery(event.currentTarget.value)}
    type="search"
    placeholder="Find in shown lines"
    spellcheck="false"
    aria-label="Find in the lines shown"
    {onkeydown}
  />
  <span class="count tabular">
    {#if find.applied.trim() === ""}
      &nbsp;
    {:else if find.hits.length === 0}
      no matches
    {:else}
      {find.at + 1} / {find.hits.length}
    {/if}
  </span>
  <button
    type="button"
    disabled={find.hits.length === 0}
    title="Previous match (Shift+Enter)"
    onclick={() => find.go(-1, reveal)}>▲</button
  >
  <button
    type="button"
    disabled={find.hits.length === 0}
    title="Next match (Enter)"
    onclick={() => find.go(1, reveal)}>▼</button
  >
  <button type="button" title="Close (Escape)" onclick={() => find.close()}>✕</button>
</div>

<style>
  .find {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    padding: var(--sp-2) var(--sp-4);
    border-bottom: 1px solid var(--divider);
    background: var(--surface-raised);
  }

  .find input {
    flex: 1 1 auto;
    min-width: 0;
    height: 22px;
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
  }

  /* Nothing found: a tint, not an error — the user is still typing (#12). */
  .find input.missing {
    border-color: var(--status-delete);
    background: color-mix(in srgb, var(--status-delete) 14%, var(--surface-input));
  }

  .find button {
    height: var(--h-button-sm);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  .find button:disabled {
    color: var(--text-secondary);
  }

  .count {
    flex: 0 0 auto;
    min-width: 64px;
    color: var(--text-secondary);
    font-size: 11px;
    text-align: right;
  }
</style>
