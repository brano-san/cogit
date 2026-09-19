<script lang="ts">
  import { repository } from "$stores/repository.svelte";

  interface Props {
    onopen: () => void;
  }

  let { onopen }: Props = $props();
  const repo = $derived(repository.current);
</script>

<div class="wrapper">
  <button type="button" class="open" onclick={onopen} disabled={repository.busy}>
    {repository.busy ? "Opening…" : "Open Repository…"}
  </button>

  {#if repo}
    <div class="row selected" title={repo.root}>
      <span class="icon" aria-hidden="true">🗁</span>
      <span class="name truncate">{repo.name}</span>
      {#if repo.isBare}<span class="badge">bare</span>{/if}
    </div>
    <div class="path truncate" title={repo.root}>{repo.root}</div>
  {:else if repository.error}
    <p class="error">{repository.error.message}</p>
  {:else}
    <p class="empty">No repository open.</p>
  {/if}
</div>

<style>
  .wrapper {
    padding: var(--sp-4) 0;
  }

  .open {
    display: block;
    width: calc(100% - var(--sp-5) * 2);
    margin: 0 var(--sp-5) var(--sp-4);
    height: var(--h-input);
    border: 1px solid var(--field-border);
    border-radius: var(--r-md);
    background: var(--surface-input);
    color: var(--text-primary);
    font: inherit;
    font-size: var(--fs-dense);
    cursor: pointer;
    transition: background var(--t-fast) var(--ease-out);
  }

  .open:hover:not(:disabled) {
    background: var(--state-hover);
  }

  .open:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: var(--h-row);
    padding: 0 var(--sp-5);
    font-size: var(--fs-dense);
    position: relative;
  }

  .row.selected {
    background: var(--state-selected);
  }

  .row.selected::before {
    content: "";
    position: absolute;
    inset-block: 0;
    inset-inline-start: 0;
    width: 2px;
    background: var(--status-ref);
  }

  .name {
    flex: 1 1 auto;
    min-width: 0;
  }

  .badge {
    padding: 0 var(--sp-3);
    border: 1px solid var(--status-stash);
    border-radius: var(--r-sm);
    color: var(--status-stash);
    background: var(--c-stash-bg);
    font-size: 10px;
    line-height: 14px;
  }

  .path {
    padding: var(--sp-1) var(--sp-5) 0 calc(var(--sp-5) + 20px);
    font-size: 11px;
    color: var(--text-secondary);
  }

  .empty,
  .error {
    margin: 0;
    padding: var(--sp-4) var(--sp-5);
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }

  .error {
    color: var(--status-delete);
    user-select: text;
  }
</style>
