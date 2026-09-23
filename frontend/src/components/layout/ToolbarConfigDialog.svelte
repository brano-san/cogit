<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import { DEFAULT_LAYOUT, SEPARATOR, actionOf } from "$lib/toolbar";
  import {
    addEntry,
    hiddenActions,
    moveEntry,
    removeEntry,
    sameLayout,
  } from "$lib/toolbar-layout";
  import { toolbar } from "$stores/toolbar.svelte";

  /** Edit ▸ Configure Toolbar…: the button toolbar, not the menu bar (#44). Edits a draft;
      OK keeps it. */
  // svelte-ignore state_referenced_locally
  let draft = $state.raw<string[]>([...toolbar.layout]);
  let shownAt = $state<number | null>(null);
  let pick = $state<string | null>(null);

  const available = $derived([
    ...hiddenActions(draft).map((action) => ({ id: action.id, label: action.label })),
    { id: SEPARATOR, label: "Separator" },
  ]);

  function labelOf(entry: string): string {
    return entry === SEPARATOR ? "Separator" : (actionOf(entry)?.label ?? entry);
  }

  function add(entry = pick) {
    if (entry === null) return;
    draft = addEntry(draft, entry, shownAt);
    shownAt = shownAt === null ? draft.length - 1 : shownAt + 1;
    pick = null;
  }

  function remove(index = shownAt) {
    if (index === null) return;
    draft = removeEntry(draft, index);
    shownAt = draft.length === 0 ? null : Math.min(index, draft.length - 1);
  }

  function move(delta: -1 | 1) {
    if (shownAt === null) return;
    const next = moveEntry(draft, shownAt, delta);
    if (sameLayout(next, draft)) return;
    draft = next;
    shownAt += delta;
  }

  function reset() {
    draft = [...DEFAULT_LAYOUT];
    shownAt = null;
    pick = null;
  }

  function close() {
    toolbar.configuring = false;
  }

  function accept() {
    void toolbar.setLayout(draft);
    close();
  }
</script>

<Dialog title="Configure Toolbar" width="min(560px, 92vw)" onclose={close} onconfirm={accept}>
  <p class="lead">Choose the buttons of the toolbar and their order.</p>
  <div class="columns">
    <section>
      <h3 id="toolbar-available">Available</h3>
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
      <button type="button" class="btn" disabled={shownAt === null} onclick={() => remove()}
        >← Remove</button
      >
      <button
        type="button"
        class="btn"
        disabled={shownAt === null || shownAt === 0}
        onclick={() => move(-1)}>Move Up</button
      >
      <button
        type="button"
        class="btn"
        disabled={shownAt === null || shownAt === draft.length - 1}
        onclick={() => move(1)}>Move Down</button
      >
    </div>

    <section>
      <h3 id="toolbar-shown">Toolbar</h3>
      <ul class="list" role="listbox" aria-labelledby="toolbar-shown">
        {#each draft as entry, index (`${entry}:${index}`)}
          <li>
            <button
              type="button"
              role="option"
              aria-selected={shownAt === index}
              class:selected={shownAt === index}
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

  {#snippet footer()}
    <button
      type="button"
      class="btn"
      disabled={sameLayout(draft, DEFAULT_LAYOUT)}
      onclick={reset}>Reset to Default</button
    >
    <span class="grow"></span>
    <button type="button" class="btn" onclick={close}>Cancel</button>
    <button type="button" class="btn primary" onclick={accept}>OK</button>
  {/snippet}
</Dialog>

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

  h3 {
    margin: 0 0 var(--sp-2);
    font-size: var(--fs-dense);
    font-weight: 600;
  }

  .list {
    height: 260px;
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

  .grow {
    flex: 1 1 auto;
  }
</style>
