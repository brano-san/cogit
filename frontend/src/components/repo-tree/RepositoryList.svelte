<script lang="ts">
  import type { RepoOverview } from "$lib/ipc";
  import { repository } from "$stores/repository.svelte";

  interface Props {
    onopen: () => void;
    onselect: (entry: RepoOverview) => void;
    onclose: (entry: RepoOverview) => void;
  }

  let { onopen, onselect, onclose }: Props = $props();

  const active = $derived(repository.current?.repo);
  const entries = $derived(repository.openRepos);
</script>

<div class="wrapper">
  <button type="button" class="open" onclick={onopen} disabled={repository.busy}>
    {repository.busy ? "Opening…" : "Open Repository…"}
  </button>

  {#if entries.length === 0}
    {#if repository.error}
      <p class="error">{repository.error.message}</p>
    {:else}
      <p class="empty">No repository open.</p>
    {/if}
  {:else}
    {#each entries as entry (entry.repo)}
      <div
        class="row"
        class:selected={active?.valueOf() === entry.repo.valueOf()}
        role="button"
        tabindex="0"
        title={entry.root}
        onclick={() => onselect(entry)}
        onkeydown={(event) => event.key === "Enter" && onselect(entry)}
      >
        <span class="icon" aria-hidden="true">🗁</span>
        <span class="name truncate">{entry.name}</span>
        {#if entry.dirty}<span class="dirty" title="Uncommitted changes">●</span>{/if}
        {#if entry.branch}<span class="branch truncate">{entry.branch}</span>{/if}
        {#if entry.ahead > 0 || entry.behind > 0}
          <span class="track tabular"
            >{entry.ahead > 0 ? "↑" + entry.ahead : ""}{entry.behind > 0
              ? "↓" + entry.behind
              : ""}</span
          >
        {/if}
        <span
          class="act"
          role="button"
          tabindex="-1"
          title="Close {entry.name}"
          onclick={(event) => {
            event.stopPropagation();
            onclose(entry);
          }}
          onkeydown={(event) => event.key === "Enter" && onclose(entry)}>✕</span
        >
      </div>
    {/each}
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
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  .open:not(:disabled):hover {
    border-color: var(--status-ref);
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: 22px;
    padding: 0 var(--sp-5);
    font-size: var(--fs-dense);
    white-space: nowrap;
  }

  .row:hover {
    background: var(--state-hover);
  }

  .row.selected {
    background: var(--state-selected);
  }

  .icon {
    flex: 0 0 auto;
  }

  .name {
    flex: 0 1 auto;
    min-width: 0;
  }

  .dirty {
    flex: 0 0 auto;
    color: var(--status-modify);
    font-size: 9px;
  }

  .branch {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--text-secondary);
    font-size: 10px;
  }

  .track {
    flex: 0 0 auto;
    color: var(--status-ref);
    font-size: 10px;
  }

  .act {
    flex: 0 0 auto;
    padding: 0 var(--sp-2, 3px);
    color: var(--text-secondary);
    opacity: 0;
    cursor: default;
  }

  .row:hover .act {
    opacity: 1;
  }

  .act:hover {
    color: var(--status-delete);
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
