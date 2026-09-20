<script lang="ts">
  import type { RepoOverview } from "$lib/ipc";
  import { repository } from "$stores/repository.svelte";

  interface Props {
    /** Only the folder dialog changes the label; selecting a repository must not (R-35). */
    opening?: boolean;
    onopen: () => void;
    onscan: () => void;
    onselect: (entry: RepoOverview) => void;
    onclose: (entry: RepoOverview) => void;
    oncontext: (entry: RepoOverview, x: number, y: number) => void;
  }

  let { opening = false, onopen, onscan, onselect, onclose, oncontext }: Props = $props();

  const active = $derived(repository.current?.repo);
  const entries = $derived(repository.openRepos);
</script>

<div class="wrapper">
  <div class="actions">
    <button type="button" class="open" onclick={onopen} disabled={repository.busy}>
      {opening ? "Opening…" : "Open Repository…"}
    </button>
    <button type="button" class="open scan" onclick={onscan} disabled={repository.busy}>
      Scan Folder…
    </button>
  </div>

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
        oncontextmenu={(event) => {
          event.preventDefault();
          oncontext(entry, event.clientX, event.clientY);
        }}
      >
        <svg class="folder" viewBox="0 0 16 16" aria-hidden="true"
          ><path
            fill="currentColor"
            d="M1.5 3.5c0-.69.56-1.25 1.25-1.25h3.04c.4 0 .78.19 1.01.51l.79 1.09h5.66c.69 0 1.25.56 1.25 1.25v7.15c0 .69-.56 1.25-1.25 1.25H2.75c-.69 0-1.25-.56-1.25-1.25V3.5Z"
          /></svg
        >
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

  .actions {
    display: flex;
    gap: var(--sp-3);
    margin: 0 var(--sp-5) var(--sp-4);
  }

  .open {
    flex: 1 1 auto;
    min-width: 0;
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

  .scan {
    flex: 0 0 auto;
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

  .folder {
    flex: 0 0 auto;
    width: 13px;
    height: 13px;
    color: var(--status-ref);
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
