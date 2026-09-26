<script lang="ts">
  import { untrack } from "svelte";
  import { formatQuery, parseQuery, sameQuery } from "$lib/query";
  import type { CommitQuery } from "$lib/ipc";
  import FilterPatterns from "$components/graph/FilterPatterns.svelte";
  import { graphFilter } from "$stores/graph-filter.svelte";

  interface Props {
    onchange: (query: CommitQuery) => void;
    matches?: number;
    /** The filter the graph is loaded with, wherever it was set: here or by File ▸ Log. */
    query: CommitQuery;
  }

  let { onchange, matches, query }: Props = $props();

  // svelte-ignore state_referenced_locally
  graphFilter.text = formatQuery(query);

  const active = $derived(!sameQuery(query, parseQuery("")));
  const fields = $derived(graphFilter.fields);

  // One source for the field and the graph: a filter set elsewhere shows here, with its
  // count and ✕, instead of an empty field over a filtered graph.
  $effect(() => {
    const now = query;
    untrack(() => {
      if (!sameQuery(parseQuery(graphFilter.text, graphFilter.fields), now)) graphFilter.text = formatQuery(now);
    });
  });

  // A switch changes where the text is looked for: the graph follows at once.
  $effect(() => {
    const on = fields;
    untrack(() => {
      if (query.text == null) return;
      const next = parseQuery(graphFilter.text, on);
      if (!sameQuery(next, query)) onchange(next);
    });
  });

  function apply() {
    const next = parseQuery(graphFilter.text, fields);
    if (!sameQuery(next, query)) onchange(next);
  }

  function usePattern(pattern: string) {
    graphFilter.text = pattern;
    apply();
  }

  function reset() {
    graphFilter.text = "";
    if (active) onchange(parseQuery(""));
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Enter") apply();
    if (event.key === "Escape" && (graphFilter.text !== "" || active)) reset();
  }
</script>

<div class="filter" class:active>
  <div class="box">
    <FilterPatterns onpick={usePattern} />
    <input
    type="search"
    bind:value={graphFilter.text}
    {onkeydown}
    onblur={apply}
    placeholder="Filter"
    aria-label="Filter commits"
    title="Looks for the text in the fields switched on under the field. author:, path:, oid:, since: and until: take a value of their own."
    />
  </div>
  {#if active}
    <span class="count tabular">{matches ?? 0}</span>
    <button type="button" onclick={reset} title="Clear filter (Esc)">✕</button>
  {/if}
  {#if graphFilter.text.trim() !== "" && !sameQuery(parseQuery(graphFilter.text, fields), query)}
    <span class="hint">Enter</span>
  {/if}
</div>

<style>
  .filter {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }

  /* The magnifier sits inside the field, as in SmartGit. */
  .box {
    display: flex;
    align-items: center;
    width: 260px;
    background: var(--surface-input);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
  }

  input {
    flex: 1 1 auto;
    min-width: 0;
    height: calc(var(--h-button-sm) - 2px);
    padding: 0 var(--sp-3) 0 0;
    background: none;
    color: var(--text-primary);
    border: 0;
    font-size: var(--fs-dense);
  }

  .filter.active .box {
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
