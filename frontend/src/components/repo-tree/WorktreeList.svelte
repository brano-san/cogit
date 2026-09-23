<script lang="ts">
  import KindIcon from "$components/common/KindIcon.svelte";
  import type { WorktreeEntry } from "$lib/ipc";
  import { worktreeTags, worktreeWhere } from "$lib/worktree-list";

  /** The rows of the Worktrees panel: one per checkout, the active one marked, a missing
      one with its two ways out right in the row (doc/12-risks.md, R-184). */
  interface Props {
    entries: readonly WorktreeEntry[];
    selected: string | null;
    onselect: (entry: WorktreeEntry) => void;
    onopen: (entry: WorktreeEntry) => void;
    oncontext: (entry: WorktreeEntry, x: number, y: number) => void;
    onprune: (entry: WorktreeEntry) => void;
    onrepair: (entry: WorktreeEntry) => void;
    onadd: () => void;
  }

  let { entries, selected, onselect, onopen, oncontext, onprune, onrepair, onadd }: Props =
    $props();
</script>

<div class="list" role="listbox" aria-label="Worktrees">
  {#each entries as entry (entry.path)}
    {@const where = worktreeWhere(entry)}
    <div
      class="row"
      class:selected={selected === entry.path}
      class:current={entry.isCurrent}
      class:missing={entry.missing}
      role="option"
      aria-selected={selected === entry.path}
      tabindex="0"
      title={entry.path}
      onclick={() => onselect(entry)}
      ondblclick={() => !entry.missing && onopen(entry)}
      onkeydown={(event) => {
        if (event.key === "Enter" && !entry.missing) onopen(entry);
      }}
      oncontextmenu={(event) => {
        event.preventDefault();
        onselect(entry);
        oncontext(entry, event.clientX, event.clientY);
      }}
    >
      <KindIcon kind="worktree" title="Worktree — {entry.path}" />
      <span class="name truncate shrink-last">{entry.name}</span>
      {#if where}<span class="where truncate shrink-first">{where}</span>{/if}
      {#each worktreeTags(entry) as tag (tag.id)}
        <span class="tag {tag.id}" title={tag.tooltip}>{tag.label}</span>
      {/each}
      {#if entry.missing}
        <span class="grow"></span>
        <button
          type="button"
          class="inline"
          title="Forget this registration; nothing on disk is touched"
          onclick={(event) => {
            event.stopPropagation();
            onprune(entry);
          }}>Prune</button
        >
        <button
          type="button"
          class="inline"
          title="Locate the folder where it is now and point Git at it"
          onclick={(event) => {
            event.stopPropagation();
            onrepair(entry);
          }}>Repair</button
        >
      {/if}
    </div>
  {/each}

  {#if entries.length <= 1}
    <div class="empty">
      <p>No linked worktrees.</p>
      <button type="button" class="btn" onclick={onadd}>Add Worktree…</button>
    </div>
  {/if}
</div>

<style>
  .list {
    padding: var(--sp-3) 0;
    overflow-y: auto;
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

  .row.selected {
    background: var(--state-selected);
  }

  .row.current {
    box-shadow: inset 2px 0 0 var(--status-ref);
  }

  .row.current .name {
    color: var(--status-ref);
    font-weight: 600;
  }

  .where {
    color: var(--text-secondary);
  }

  .row.missing .name,
  .row.missing .where {
    color: var(--text-secondary);
    text-decoration: line-through;
    opacity: 0.75;
  }

  .tag {
    flex: none;
    padding: 0 var(--sp-2);
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
    color: var(--text-secondary);
    font-size: 10px;
    line-height: 14px;
  }

  .tag.missing {
    color: var(--status-delete);
    border-color: var(--status-delete);
  }

  .tag.dirty {
    color: var(--status-modify);
    border-color: var(--status-modify);
  }

  .grow {
    flex: 1 1 auto;
  }

  .inline {
    flex: none;
    height: 18px;
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: 10px;
    cursor: default;
  }

  .inline:hover {
    border-color: var(--status-ref);
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-5);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .empty p {
    margin: 0;
  }

  .btn {
    height: var(--h-button-sm);
    padding: 0 var(--sp-4);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  .btn:hover {
    border-color: var(--status-ref);
  }
</style>
