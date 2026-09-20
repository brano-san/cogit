<script lang="ts">
  import { settings } from "$stores/settings.svelte";
  import type { StashEntry } from "$lib/ipc";

  interface Props {
    stashes: readonly StashEntry[];
    onapply: (index: number, pop: boolean) => void;
    ondrop: (index: number) => void;
  }

  let { stashes, onapply, ondrop }: Props = $props();
</script>

{#if stashes.length > 0}
  <div class="section">
    <div class="section-header">Stashes ({stashes.length})</div>
    {#each stashes as stash (stash.oid)}
      <div class="row" title={stash.message}>
        <span class="marker" aria-hidden="true">⚑</span>
        <span class="name truncate">{stash.message}</span>
        <span class="date tabular">{settings.formatDate(stash.timestamp, 0)}</span>
        <span
          class="act"
          role="button"
          tabindex="-1"
          title="Apply and keep the stash"
          onclick={() => onapply(stash.index, false)}
          onkeydown={(e) => e.key === "Enter" && onapply(stash.index, false)}>Apply</span
        >
        <span
          class="act"
          role="button"
          tabindex="-1"
          title="Apply and remove the stash"
          onclick={() => onapply(stash.index, true)}
          onkeydown={(e) => e.key === "Enter" && onapply(stash.index, true)}>Pop</span
        >
        <span
          class="act"
          role="button"
          tabindex="-1"
          title="Drop the stash"
          onclick={() => ondrop(stash.index)}
          onkeydown={(e) => e.key === "Enter" && ondrop(stash.index)}>Drop</span
        >
      </div>
    {/each}
  </div>
{/if}

<style>
  .section {
    padding-bottom: var(--sp-4);
  }

  .section-header {
    padding: var(--sp-3) var(--sp-5) var(--sp-2, 3px);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
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

  .marker {
    flex: 0 0 auto;
    width: 10px;
    color: var(--status-stash);
  }

  .name {
    flex: 1 1 auto;
    min-width: 0;
  }

  .date {
    flex: 0 0 auto;
    color: var(--text-secondary);
    font-size: 10px;
  }

  .act {
    flex: 0 0 auto;
    padding: 0 var(--sp-3);
    color: var(--text-secondary);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0;
    cursor: default;
  }

  .row:hover .act {
    opacity: 1;
  }

  .act:hover {
    color: var(--status-ref);
  }
</style>
