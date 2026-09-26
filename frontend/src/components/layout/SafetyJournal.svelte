<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
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
</script>

<Dialog title="Safety journal" {onclose} width="min(640px, 92vw)" flush>
  <div class="rows">
    {#if entries.length === 0}
      <p class="empty">Nothing destructive has happened in this repository yet.</p>
    {:else}
      {#each entries as entry (entry.id)}
        <div class="row" class:spent={!entry.undoable}>
          <span class="what truncate" title={entry.description}>{entry.description}</span>
          {#if entry.undoable}
            <button type="button" class="btn" disabled={busy} onclick={() => onundo(entry)}>Undo</button>
          {:else}
            <span class="note">cannot be undone</span>
          {/if}
        </div>
      {/each}
    {/if}
  </div>

  {#snippet footer()}
    <span class="hint">
      Newest first. Each entry restores its own thing, so an older one can be undone first.
    </span>
    <button type="button" class="btn primary" data-autofocus onclick={onclose}>Close</button>
  {/snippet}
</Dialog>

<style>
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
    padding: var(--sp-2) var(--dialog-inset);
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
    padding: var(--sp-6) var(--dialog-inset);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .hint {
    flex: 1 1 auto;
    color: var(--text-secondary);
    font-size: 11px;
  }
</style>
