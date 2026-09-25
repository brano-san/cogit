
<script lang="ts">
  import { untrack } from "svelte";
  import { formatQuery, parseQuery, sameQuery } from "$lib/query";
  import type { CommitQuery } from "$lib/ipc";

  interface Props {
    onchange: (query: CommitQuery) => void;
    matches?: number;
    /** The filter the graph is loaded with, wherever it was set: here or by File ▸ Log. */
    query: CommitQuery;
  }

  let { onchange, matches, query }: Props = $props();

  const PLACEHOLDER = "Filter: author:brano path:src since:2026-01-01 free text";

  // svelte-ignore state_referenced_locally
  let text = $state(formatQuery(query));

  const active = $derived(!sameQuery(query, parseQuery("")));

  // One source for the field and the graph: a filter set elsewhere shows here, with its
  // count and ✕, instead of an empty field over a filtered graph.
  $effect(() => {
    const now = query;
    untrack(() => {
      if (!sameQuery(parseQuery(text), now)) text = formatQuery(now);
    });
  });

  function apply() {
    const next = parseQuery(text);
    if (!sameQuery(next, query)) onchange(next);
  }

  function reset() {
    text = "";
    if (active) onchange(parseQuery(""));
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Enter") apply();
    if (event.key === "Escape" && (text !== "" || active)) reset();
  }
</script>

<div class="filter" class:active>
  <input
    type="search"
    bind:value={text}
    {onkeydown}
    onblur={apply}
    placeholder={PLACEHOLDER}
    aria-label="Filter commits"
  />
  {#if active}
    <span class="count tabular">{matches ?? 0}</span>
    <button type="button" onclick={reset} title="Clear filter (Esc)">✕</button>
  {/if}
  {#if text.trim() !== "" && !sameQuery(parseQuery(text), query)}
    <span class="hint">Enter</span>
  {/if}
</div>

<style>
  .filter {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }

  input {
    width: 260px;
    height: var(--h-button-sm);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
  }

  .filter.active input {
    border-color: var(--status-ref);
  }

  .count {
    color: var(--status-ref);
    font-size: 11px;
  }

  .hint {
    color: var(--text-secondary);
    font-size: 10px;
  }

  button {
    height: 16px;
    padding: 0 var(--sp-2);
    background: none;
    color: var(--text-secondary);
    border: 0;
    cursor: default;
  }

  button:hover {
    color: var(--text-primary);
  }
</style>
