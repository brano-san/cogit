<script lang="ts">
  import { SEPARATOR, actionOf } from "$lib/toolbar";
  import { addEntry, hiddenActions, moveEntry, removeEntry, sameLayout } from "$lib/toolbar-layout";

  /** Preferences ▸ Toolbar: the button toolbar, not the menu bar. Every edit is applied to
      the toolbar at once; Undo takes the edits of this window back one by one. */
  interface Props {
    layout: readonly string[];
    onchange: (next: string[]) => void;
    canUndo: boolean;
    onundo: () => void;
  }

  let { layout, onchange, canUndo, onundo }: Props = $props();

  let shownAt = $state<number | null>(null);
  let pick = $state<string | null>(null);

  const available = $derived([
    ...hiddenActions(layout).map((action) => ({ id: action.id, label: action.label })),
    { id: SEPARATOR, label: "Separator" },
  ]);

  /** Undo or Restore Defaults can shorten the list under the selection. */
  const selected = $derived(shownAt !== null && shownAt < layout.length ? shownAt : null);

  function labelOf(entry: string): string {
    return entry === SEPARATOR ? "Separator" : (actionOf(entry)?.label ?? entry);
  }

  function add(entry = pick) {
    if (entry === null) return;
    const next = addEntry(layout, entry, selected);
    shownAt = selected === null ? next.length - 1 : selected + 1;
    pick = null;
    onchange(next);
  }

  function remove(index = selected) {
    if (index === null) return;
    const next = removeEntry(layout, index);
    shownAt = next.length === 0 ? null : Math.min(index, next.length - 1);
    onchange(next);
  }

  function move(delta: -1 | 1) {
    if (selected === null) return;
    const next = moveEntry(layout, selected, delta);
    if (sameLayout(next, layout)) return;
    shownAt = selected + delta;
    onchange(next);
  }
</script>

<div class="editor">
  <p class="lead">Choose the buttons of the toolbar and their order. Changes show at once.</p>
  <div class="columns">
    <section>
      <h4 id="toolbar-available">Available</h4>
      <ul class="list" role="listbox" aria-labelledby="toolbar-available">
        {#each available as entry (entry.id)}
          <li>
            <button
              type="button"
              role="option"
              aria-selected={pick === entry.id}
              class:selected={pick === entry.id}
              onclick={() => (pick = entry.id)}
              ondblclick={() => add(entry.id)}>{entry.label}</button
            >
          </li>
        {/each}
      </ul>
    </section>

    <div class="moves">
      <button type="button" class="btn" disabled={pick === null} onclick={() => add()}
        >Add →</button
      >
      <button type="button" class="btn" disabled={selected === null} onclick={() => remove()}
        >← Remove</button
      >
      <button
        type="button"
        class="btn"
        disabled={selected === null || selected === 0}
        onclick={() => move(-1)}>Move Up</button
      >
      <button
        type="button"
        class="btn"
        disabled={selected === null || selected === layout.length - 1}
        onclick={() => move(1)}>Move Down</button
      >
      <button
        type="button"
        class="btn"
        title="Take back the last change made to the toolbar in this window"
        disabled={!canUndo}
        onclick={onundo}>Undo</button
      >
    </div>

    <section>
      <h4 id="toolbar-shown">Toolbar</h4>
      <ul class="list" role="listbox" aria-labelledby="toolbar-shown">
        {#each layout as entry, index (`${entry}:${index}`)}
          <li>
            <button
              type="button"
              role="option"
              aria-selected={selected === index}
              class:selected={selected === index}
              class:separator={entry === SEPARATOR}
              onclick={() => (shownAt = index)}
              ondblclick={() => remove(index)}>{labelOf(entry)}</button
            >
          </li>
        {:else}
          <li class="empty">No buttons</li>
        {/each}
      </ul>
    </section>
  </div>
</div>

<style>
  .lead {
    margin: 0 0 var(--sp-4);
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }

  .columns {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    gap: var(--sp-5);
    align-items: stretch;
  }

  h4 {
    margin: 0 0 var(--sp-2);
    font-size: var(--fs-dense);
    font-weight: 600;
  }

  .list {
    height: 300px;
    margin: 0;
    padding: var(--sp-1) 0;
    overflow-y: auto;
    list-style: none;
    background: var(--surface-input);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
  }

  .list button {
    display: block;
    width: 100%;
    padding: var(--sp-1) var(--sp-4);
    background: none;
    border: 0;
    color: var(--text-primary);
    font: inherit;
    font-size: var(--fs-dense);
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: default;
  }

  .list button:hover {
    background: var(--state-hover);
  }

  .list button.selected {
    background: var(--state-selected);
  }

  .list button.separator {
    color: var(--text-secondary);
    font-style: italic;
  }

  .empty {
    padding: var(--sp-2) var(--sp-4);
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }

  .moves {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: var(--sp-3);
  }
</style>
