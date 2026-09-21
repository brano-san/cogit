<script lang="ts">
  import type { WorktreeEntry } from "$lib/ipc";

  /** Worktrees sit beside the repository, not behind a dialog: they are things the user
      switches between, and the one they are in has to be obvious (T3.8). */
  interface Props {
    entries: readonly WorktreeEntry[];
    onopen: (entry: WorktreeEntry) => void;
    onremove: (entry: WorktreeEntry) => void;
    onadd: () => void;
    onprune: () => void;
  }

  let { entries, onopen, onremove, onadd, onprune }: Props = $props();

  const stale = $derived(entries.some((entry) => entry.missing));
</script>

{#if entries.length > 1 || stale}
  <div class="section">
    <span class="grow">Worktrees ({entries.length})</span>
    <button type="button" class="act" title="Add a worktree" onclick={onadd}>Add</button>
    {#if stale}
      <button type="button" class="act" title="Forget the missing ones" onclick={onprune}>
        Prune
      </button>
    {/if}
  </div>

  {#each entries as entry (entry.path)}
    <div class="row" class:current={entry.isCurrent} class:missing={entry.missing}>
      <span class="marker" aria-hidden="true">{entry.isCurrent ? "▶" : "▷"}</span>
      <button
        type="button"
        class="name truncate"
        title={entry.path}
        disabled={entry.missing}
        onclick={() => onopen(entry)}
      >
        {entry.branch ?? `detached at ${entry.head.slice(0, 7)}`}
      </button>

      {#if entry.missing}
        <span class="tag gone" title="The folder is no longer there">missing</span>
      {:else if entry.dirty}
        <span class="dirty" title="Uncommitted changes">●</span>
      {/if}
      {#if entry.locked !== null}
        <span class="tag" title={entry.locked || "Locked without a reason"}>locked</span>
      {/if}
      {#if entry.isMain}
        <span class="tag">main</span>
      {:else}
        <span
          class="act"
          role="button"
          tabindex="-1"
          title="Remove this worktree"
          onclick={() => onremove(entry)}
          onkeydown={(event) => event.key === "Enter" && onremove(entry)}>✕</span
        >
      {/if}
    </div>
  {/each}
{/if}

<style>
  .section {
    display: flex;
    align-items: center;
    height: var(--h-row-dense);
    padding: 0 var(--sp-5);
    background: var(--surface-raised);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
  }

  .grow {
    flex: 1 1 auto;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: var(--h-row-dense);
    padding: 0 var(--sp-5);
    font-size: var(--fs-dense);
    white-space: nowrap;
  }

  .row:hover {
    background: var(--state-hover);
  }

  .row.current {
    background: var(--state-selected);
  }

  .marker {
    flex: 0 0 auto;
    color: var(--status-ref);
    font-size: 9px;
  }

  .name {
    flex: 0 1 auto;
    min-width: 0;
    background: none;
    border: 0;
    padding: 0;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: default;
  }

  .row.missing .name {
    color: var(--text-secondary);
    text-decoration: line-through;
  }

  .dirty {
    flex: 0 0 auto;
    color: var(--status-modify);
    font-size: 9px;
  }

  .tag {
    flex: 0 0 auto;
    padding: 0 var(--sp-2);
    border-radius: var(--r-sm);
    background: var(--state-selected);
    color: var(--text-secondary);
    font-size: 10px;
  }

  .tag.gone {
    color: var(--status-delete);
  }

  .act {
    flex: 0 0 auto;
    margin-left: auto;
    padding: 0 var(--sp-3);
    background: none;
    border: 0;
    color: var(--text-secondary);
    font: inherit;
    font-size: 10px;
    letter-spacing: 0.04em;
    cursor: default;
  }

  .act:hover {
    color: var(--status-ref);
  }

  .section .act {
    margin-left: 0;
  }
</style>
