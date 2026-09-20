<script lang="ts">
  import { isEmptyQuery, parseQuery } from "$lib/query";
  import type { CommitQuery } from "$lib/ipc";

  interface Props {
    onchange: (query: CommitQuery) => void;
    matches?: number;
  }

  let { onchange, matches }: Props = $props();

  const PLACEHOLDER = "Filter: author:brano path:src since:2026-01-01 free text";

  let text = $state("");
  let applied = $state("");

  const active = $derived(applied.trim() !== "");

  function apply() {
    if (text === applied) return;
    applied = text;
    onchange(parseQuery(text));
  }

  function reset() {
    text = "";
    applied = "";
    onchange(parseQuery(""));
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Enter") apply();
    if (event.key === "Escape" && text !== "") reset();
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
  {:else if !isEmptyQuery(parseQuery(text)) && text !== ""}
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
    height: 20px;
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
    padding: 0 var(--sp-2, 3px);
    background: none;
    color: var(--text-secondary);
    border: 0;
    cursor: default;
  }

  button:hover {
    color: var(--text-primary);
  }
</style>
