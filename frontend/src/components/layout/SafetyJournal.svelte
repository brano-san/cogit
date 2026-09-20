<script lang="ts">
  import type { SafetyEntry } from "$lib/ipc";

  /** Every destructive operation and the way back from it. Until now only the newest one
      was reachable, through the Undo button's tooltip (T5.7). */
  interface Props {
    entries: readonly SafetyEntry[];
    busy: boolean;
    onundo: (entry: SafetyEntry) => void;
    onclose: () => void;
  }

  let { entries, busy, onundo, onclose }: Props = $props();

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    }
  }
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose}></div>

<div class="dialog" role="dialog" aria-label="Safety journal">
  <header>
    <h2>Safety journal</h2>
    <button type="button" class="icon" onclick={onclose} aria-label="Close">✕</button>
  </header>

  <div class="rows">
    {#if entries.length === 0}
      <p class="empty">Nothing destructive has happened in this repository yet.</p>
    {:else}
      {#each entries as entry (entry.id)}
        <div class="row" class:spent={!entry.undoable}>
          <span class="what truncate" title={entry.description}>{entry.description}</span>
          {#if entry.undoable}
            <button type="button" disabled={busy} onclick={() => onundo(entry)}>Undo</button>
          {:else}
            <span class="note">cannot be undone</span>
          {/if}
        </div>
      {/each}
    {/if}
  </div>

  <footer>
    <span class="hint">
      Newest first. Each entry restores its own thing, so an older one can be undone first.
    </span>
    <button type="button" onclick={onclose}>Close</button>
  </footer>
</div>

<style>
  .backdrop {
    position: absolute;
    inset: 0;
    z-index: 20;
    background: rgb(0 0 0 / 35%);
  }

  .dialog {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 21;
    display: flex;
    flex-direction: column;
    width: min(640px, 92vw);
    max-height: 80vh;
    background: var(--surface-panel);
    border: 1px solid var(--field-border);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-popover);
    overflow: hidden;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex: 0 0 auto;
    height: var(--h-toolbar);
    padding: 0 var(--sp-5);
    background: var(--titlebar-bg);
    border-bottom: 1px solid var(--titlebar-border);
  }

  h2 {
    margin: 0;
    font-size: var(--fs-ui);
    font-weight: 600;
  }

  .icon {
    background: none;
    border: 0;
    color: var(--text-secondary);
    font: inherit;
    cursor: default;
  }

  .rows {
    flex: 1 1 auto;
    min-height: 80px;
    overflow: auto;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    min-height: var(--h-row);
    padding: var(--sp-2) var(--sp-5);
    font-size: var(--fs-dense);
  }

  .row:hover {
    background: var(--state-hover);
  }

  .row.spent {
    color: var(--text-secondary);
  }

  .what {
    flex: 1 1 auto;
    min-width: 0;
  }

  .note {
    flex: 0 0 auto;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .empty {
    margin: 0;
    padding: var(--sp-6) var(--sp-5);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  footer {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    flex: 0 0 auto;
    padding: var(--sp-4) var(--sp-5);
    background: var(--titlebar-bg);
    border-top: 1px solid var(--titlebar-border);
  }

  .hint {
    flex: 1 1 auto;
    color: var(--text-secondary);
    font-size: 11px;
  }
</style>
