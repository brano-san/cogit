<script lang="ts">
  import { tick } from "svelte";
  import Checkbox from "$components/common/Checkbox.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import {
    filterRows,
    isUnavailable,
    noteFor,
    openTarget,
    showsFilter,
    type MruRow,
    type Step,
    type WelcomeAction,
  } from "$lib/welcome";
  import type { WelcomeDialog } from "$stores/welcome.svelte";

  /** Shown only at the start of a session with nothing to restore (F-586). */
  interface Props {
    dialog: WelcomeDialog;
    rows: readonly MruRow[];
    showAtStart: boolean;
    onshowchange: (show: boolean) => void;
    onrun: (action: WelcomeAction) => void;
    onforget: (path: string) => void;
    oncontext: (path: string, x: number, y: number) => void;
    onclose: () => void;
  }

  let { dialog, rows, showAtStart, onshowchange, onrun, onforget, oncontext, onclose }: Props = $props();

  let list: HTMLDivElement | undefined = $state();
  let field: HTMLInputElement | undefined = $state();
  const filtering = $derived(showsFilter(rows.length));
  const shown = $derived(filtering ? filterRows(rows, dialog.query) : [...rows]);
  const target = $derived(openTarget(shown, dialog.selected, dialog.availability));

  const STEPS: Record<string, Step> = { ArrowDown: "down", ArrowUp: "up", Home: "home", End: "end" };

  function focusSelected() {
    void tick().then(() => list?.querySelector<HTMLElement>('[aria-selected="true"]')?.focus());
  }

  function open() {
    if (target !== null) onrun({ kind: "open", path: target });
  }

  function select(path: string) {
    dialog.select(path);
    focusSelected();
  }

  /** On the list: arrows, Home/End, Delete; with a filter, typing goes to the filter. */
  function onlistkey(event: KeyboardEvent) {
    const step = STEPS[event.key];
    if (step) {
      event.preventDefault();
      dialog.move(step, shown);
      focusSelected();
    } else if (event.key === "Delete" && dialog.selected !== null) {
      event.preventDefault();
      onforget(dialog.selected);
      focusSelected();
    } else if (filtering && field && event.key.length === 1 && !event.ctrlKey && !event.metaKey && !event.altKey) {
      event.preventDefault();
      setFilter(dialog.query + event.key);
      field.focus();
    }
  }

  /** In the filter: Up/Down walk the list without leaving the field, Enter opens. */
  function onfieldkey(event: KeyboardEvent) {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    dialog.move(event.key === "ArrowDown" ? "down" : "up", shown);
  }

  function setFilter(query: string) {
    dialog.filter(query, filterRows(rows, query));
  }
</script>

<Dialog title="Welcome to Cogit" {onclose} onconfirm={open} width="min(600px, 94vw)">
  <div class="welcome">
    <div class="actions">
      <button type="button" class="btn action" data-autofocus={rows.length === 0 ? "" : undefined} onclick={() => onrun({ kind: "folder" })}>
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path d="M1.5 4A1.5 1.5 0 0 1 3 2.5h3l1.5 1.5H13A1.5 1.5 0 0 1 14.5 5.5V12A1.5 1.5 0 0 1 13 13.5H3A1.5 1.5 0 0 1 1.5 12Z" />
        </svg>
        Open or Create…
      </button>
      <button type="button" class="btn action" onclick={() => onrun({ kind: "clone" })}>
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path d="M8 2v8M4.5 6.5 8 10l3.5-3.5M2.5 11v1.5A1 1 0 0 0 3.5 13.5h9a1 1 0 0 0 1-1V11" />
        </svg>
        Clone…
      </button>
    </div>

    <h3 id="welcome-recent">Recent repositories</h3>

    {#if rows.length === 0}
      <p class="empty">No repository has been opened yet.</p>
    {:else}
      {#if filtering}
        <input
          bind:this={field}
          type="search"
          value={dialog.query}
          oninput={(event) => setFilter(event.currentTarget.value)}
          onkeydown={onfieldkey}
          placeholder="Filter repositories"
          aria-label="Filter recent repositories"
          aria-controls="welcome-list"
        />
      {/if}
      <div
        bind:this={list}
        id="welcome-list"
        class="list"
        role="listbox"
        tabindex="-1"
        aria-labelledby="welcome-recent"
        onkeydown={onlistkey}
      >
        {#each shown as row (row.path)}
          {@const state = dialog.availability.get(row.path)}
          {@const note = noteFor(state ?? "checking")}
          {@const current = dialog.selected === row.path}
          {@const gone = isUnavailable(state)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div
            class="row"
            class:selected={current}
            class:gone
            role="option"
            aria-selected={current}
            aria-label={note ? `${row.name}, ${row.path}, ${note}` : `${row.name}, ${row.path}`}
            title={note ? `${row.path} (${note})` : row.path}
            tabindex={current ? 0 : -1}
            data-autofocus={current ? "" : undefined}
            onclick={() => select(row.path)}
            ondblclick={() => {
              if (!gone) onrun({ kind: "open", path: row.path });
            }}
            oncontextmenu={(event) => {
              event.preventDefault();
              select(row.path);
              oncontext(row.path, event.clientX, event.clientY);
            }}
          >
            {#if gone}
              <svg class="warn" viewBox="0 0 16 16" aria-hidden="true">
                <path d="M8 2 14.5 13.5h-13Z" />
                <path d="M8 6.5v3.5M8 11.8v.2" />
              </svg>
            {/if}
            <span class="name">{row.name}</span>
            <span class="path truncate">{row.path}</span>
          </div>
        {:else}
          <p class="empty">No repository matches the filter.</p>
        {/each}
      </div>
    {/if}
  </div>

  {#snippet footer()}
    <span class="show">
      <Checkbox checked={showAtStart} onchange={onshowchange} label="Show this dialog if no repository was opened" />
    </span>
    <button type="button" class="btn" onclick={onclose}>Close</button>
    <button type="button" class="btn primary" disabled={target === null} onclick={open}>Open</button>
  {/snippet}
</Dialog>

<style>
  /* Sized by its content; past the dialog's own limit the list, not the dialog, scrolls. */
  .welcome {
    display: flex;
    flex: 0 1 auto;
    flex-direction: column;
    gap: var(--sp-4);
    min-height: 0;
    font-size: var(--fs-dense);
  }

  .actions {
    display: flex;
    gap: var(--sp-4);
  }

  .action {
    display: inline-flex;
    flex: 1 1 0;
    align-items: center;
    justify-content: center;
    gap: var(--sp-3);
  }

  .action svg,
  .warn {
    flex: none;
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  h3 {
    margin: var(--sp-3) 0 0;
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .empty {
    margin: 0;
    padding: var(--sp-3) var(--sp-4);
    color: var(--text-secondary);
  }

  .list {
    display: flex;
    flex: 0 1 auto;
    flex-direction: column;
    min-height: 0;
    max-height: 360px;
    overflow-y: auto;
    background: var(--surface-input);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
  }

  .row {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--sp-3);
    height: var(--h-row);
    padding: 0 var(--sp-4);
    white-space: nowrap;
    cursor: default;
  }

  .row:hover {
    background: var(--state-hover);
  }

  .row.selected {
    background: var(--state-selected);
  }

  .name {
    flex: none;
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-primary);
    font-weight: 600;
  }

  .path {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--text-secondary);
  }

  .row.gone .name,
  .row.gone .path {
    color: var(--text-disabled);
  }

  .warn {
    color: var(--status-modify);
  }

  .show {
    flex: 1 1 auto;
    min-width: 0;
    font-size: var(--fs-header);
    color: var(--text-secondary);
  }
</style>
