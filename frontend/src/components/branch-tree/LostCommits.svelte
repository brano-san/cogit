<script lang="ts">
  import { settings } from "$stores/settings.svelte";
  import { shortOid } from "$lib/format";
  import type { CommitRow } from "$lib/ipc";

  interface Props {
    commits: readonly CommitRow[];
    onrestore: (commit: CommitRow) => void;
  }

  let { commits, onrestore }: Props = $props();
</script>

{#if commits.length > 0}
  <div class="section">
    <div class="section-header">Lost Commits ({commits.length})</div>
    {#each commits as lost (lost.oid)}
      <div class="row" title="{lost.summary} — reachable only through the reflog">
        <span class="marker" aria-hidden="true">⚠</span>
        <span class="name truncate">{lost.summary}</span>
        <span class="oid mono tabular">{shortOid(lost.oid)}</span>
        <span class="date tabular">{settings.formatDate(lost.timestamp, lost.tzOffsetMinutes)}</span>
        <span
          class="act"
          role="button"
          tabindex="-1"
          title="Create a branch here"
          onclick={() => onrestore(lost)}
          onkeydown={(e) => e.key === "Enter" && onrestore(lost)}>Recover</span
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
    color: var(--status-modify);
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
    color: var(--status-modify);
  }

  .name {
    flex: 1 1 auto;
    min-width: 0;
  }

  .oid,
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
