<script lang="ts">
  import KindIcon from "$components/common/KindIcon.svelte";
  import { striped } from "$lib/graph-geometry";
  import type { WorktreeEntry } from "$lib/ipc";
  import { listedRows, shortWorktreePath, worktreeRowKey, worktreeTags, worktreeWhere } from "$lib/worktree-list";
  import { pruneBlocked } from "$lib/worktree-menu";
  import { TypeAhead, moveFocus } from "$lib/list-keys";
  import { settings } from "$stores/settings.svelte";

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
    /** Delete on a row: the Remove dialog, which confirms. */
    onremove: (entry: WorktreeEntry) => void;
  }

  let { entries, selected, onselect, onopen, oncontext, onprune, onrepair, onremove }: Props = $props();

  let list: HTMLDivElement | undefined = $state();
  /** One switch for every list's banding, the graph's (#41). */
  const stripes = $derived(settings.current.graphStripes);
  const typing = new TypeAhead();
  const rows = $derived(listedRows(entries));
  const mainPath = $derived(entries.find((entry) => entry.isMain)?.path);

  /** 11 §10: the arrows and typing move the selection; Enter opens, as before. */
  function onkeydown(event: KeyboardEvent) {
    if (!list) return;
    const row = moveFocus(list, event, typing);
    const entry = entries.find((each) => each.path === row?.dataset.keyRow);
    if (entry) onselect(entry);
  }
</script>

<!-- Roving focus: the rows take it one at a time, the list itself never does. -->
<!-- svelte-ignore a11y_interactive_supports_focus -->
<div class="list key-list" role="listbox" aria-label="Worktrees" bind:this={list} {onkeydown}>
  {#each rows as entry, at (entry.path)}
    {@const where = worktreeWhere(entry)}
    <div
      class="row"
      class:striped={striped(at, stripes)}
      class:selected={selected === entry.path}
      class:current={entry.isCurrent}
      class:missing={entry.missing}
      role="option"
      aria-selected={selected === entry.path}
      tabindex="0"
      title={entry.path}
      data-key-row={entry.path}
      data-key-label={entry.name}
      onclick={() => onselect(entry)}
      ondblclick={() => !entry.missing && onopen(entry)}
      onkeydown={(event) => {
        const action = worktreeRowKey(event.key, entry);
        if (action === null) return;
        event.preventDefault();
        if (action === "open") onopen(entry);
        else onremove(entry);
      }}
      oncontextmenu={(event) => {
        event.preventDefault();
        onselect(entry);
        oncontext(entry, event.clientX, event.clientY);
      }}
    >
      <KindIcon kind="worktree" title="Worktree — {entry.path}" />
      <span class="name truncate" title={entry.name}>{entry.name}</span>
      <span class="where truncate" title={where ?? undefined}>{where ?? ""}</span>
      <span class="path truncate" title={entry.path}>{shortWorktreePath(entry.path, mainPath)}</span>
      <span class="tags">
        {#each worktreeTags(entry) as tag (tag.id)}
          <span class="tag {tag.id}" title={tag.tooltip}>{tag.label}</span>
        {/each}
      </span>
      <span class="actions">
      {#if entry.missing}
        {@const blocked = pruneBlocked(entry)}
        <button
          type="button"
          
          disabled={blocked !== null}
          title={blocked === null
            ? "Forget this registration; nothing on disk is touched"
            : `Git keeps a locked worktree: ${blocked} (right-click ▸ Unlock)`}
          onclick={(event) => {
            event.stopPropagation();
            onprune(entry);
          }}>Prune</button
        >
        <button
          type="button"
          
          title="Locate the folder where it is now and point Git at it"
          onclick={(event) => {
            event.stopPropagation();
            onrepair(entry);
          }}>Repair</button
        >
      {/if}
      </span>
    </div>
  {/each}

  {#if rows.length === 0}
    <!-- One line: the header's Add… is the panel's one way to add (one action, one place). -->
    <p class="empty truncate">No linked worktrees</p>
  {/if}
</div>

<style>
  /* One grid for every row, so the columns line up down the list: icon | name | branch |
     path | badges. Name and branch keep their width; the path gives way first. The actions
     float over the row's end and take no column, so a missing row does not narrow the rest. */
  .list {
    display: grid;
    grid-template-columns: auto minmax(6ch, max-content) minmax(6ch, max-content) minmax(0, 1fr) auto;
    align-content: start;
    width: 100%;
    min-width: 0;
    padding: var(--sp-3) 0;
    overflow-y: auto;
  }

  .row {
    position: relative;
    grid-column: 1 / -1;
    display: grid;
    grid-template-columns: subgrid;
    align-items: center;
    column-gap: var(--sp-3);
    height: var(--h-row-dense);
    padding: 0 var(--sp-5);
    font-size: var(--fs-dense);
    white-space: nowrap;
  }

  /* By the row's place in the list (#41), before hover and selection, which cover it. */
  .row.striped {
    background: var(--row-stripe);
  }

  .row:hover {
    background: var(--state-hover);
  }

  .row.selected {
    background: var(--state-selected);
  }

  .row.current {
    box-shadow: inset 2px 0 0 var(--selected-bar);
  }

  .row.current .name {
    color: var(--status-ref);
    font-weight: 600;
  }

  .where {
    color: var(--text-secondary);
  }

  .path {
    color: var(--text-secondary);
    opacity: 0.8;
  }

  .row.missing .name,
  .row.missing .where,
  .row.missing .path {
    color: var(--text-secondary);
    text-decoration: line-through;
    opacity: 0.75;
  }

  .tags,
  .actions {
    display: flex;
    gap: var(--sp-2);
    justify-content: flex-end;
  }

  .actions {
    position: absolute;
    inset: 0 var(--sp-5) 0 auto;
    align-items: center;
    padding-left: var(--sp-3);
    background: inherit;
  }

  .row:hover .actions {
    background: var(--state-hover);
  }

  .row.selected .actions {
    background: var(--state-selected);
  }

  /* A state, not a control: a tinted pill with no border and no hover or press, so it never
     reads as one of the buttons beside it. Neutral, accent, warning, error by meaning. */
  .tag {
    padding: 0 var(--sp-3);
    border-radius: 999px;
    background: var(--badge-remote-bg);
    color: var(--badge-remote-fg);
    font-size: 10px;
    line-height: 15px;
    cursor: default;
    user-select: none;
  }

  .tag.open {
    background: var(--badge-branch-bg);
    color: var(--badge-branch-fg);
  }

  .tag.dirty {
    background: var(--badge-warning-bg);
    color: var(--badge-warning-fg);
  }

  .tag.missing {
    background: var(--badge-error-bg);
    color: var(--badge-error-fg);
  }

  /* Real buttons, and only where they can be pressed: the hovered, focused or selected row. */
  .row:not(:hover, :focus-within, .selected) .actions {
    visibility: hidden;
  }

  .actions button {
    height: 18px;
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: 10px;
    cursor: default;
  }

  .actions button:hover:not(:disabled) {
    border-color: var(--status-ref);
  }

  .actions button:disabled {
    opacity: 0.45;
  }

  .empty {
    margin: 0;
    height: var(--h-row-dense);
    line-height: var(--h-row-dense);
    padding: 0 var(--sp-5);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }
</style>
