<script lang="ts">
  import type { RebaseProgress } from "$lib/ipc";

  interface Props {
    progress: RebaseProgress;
    changes: number;
    staged: number;
  }

  let { progress, changes, staged }: Props = $props();
</script>

<div class="stack" aria-label="Rebase in progress">
  <div class="row virtual">
    <span class="node" aria-hidden="true">●</span>
    <span class="label">Working Tree</span>
    <span class="count">{changes} changes</span>
  </div>

  <div class="row virtual">
    <span class="node" aria-hidden="true">●</span>
    <span class="label">Index</span>
    <span class="count">{staged} changes</span>
    {#if progress.applying}
      <span class="applying truncate">rebasing: {progress.applying}</span>
    {/if}
  </div>

  {#each progress.todo as step (step.oid)}
    <div class="row todo">
      <span class="node" aria-hidden="true">◌</span>
      <span class="badge">{step.action}</span>
      <span class="label truncate">{step.summary || step.oid.slice(0, 7)}</span>
    </div>
  {/each}

  <div class="row onto">
    <span class="node" aria-hidden="true">▶</span>
    <span class="label">
      Replaying onto {progress.onto ? progress.onto.slice(0, 7) : "the new base"}
    </span>
    <span class="count">{progress.done} of {progress.total} done</span>
  </div>
</div>

<style>
  .stack {
    flex: 0 0 auto;
    padding: var(--sp-2) 0;
    background: var(--c-modified-bg);
    border-bottom: 1px solid var(--divider);
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    height: var(--h-row-dense);
    padding: 0 var(--sp-5);
    font-size: var(--fs-dense);
  }

  .node {
    flex: 0 0 12px;
    color: var(--status-modify);
    text-align: center;
  }

  .row.todo {
    color: var(--text-secondary);
  }

  .row.todo .node {
    color: var(--text-secondary);
  }

  .row.onto .node {
    color: var(--status-ref);
  }

  .label {
    min-width: 0;
  }

  .count,
  .applying {
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  .applying {
    flex: 1 1 auto;
    min-width: 0;
  }

  .badge {
    flex: 0 0 auto;
    padding: 0 var(--sp-2);
    background: var(--surface-raised);
    border-radius: var(--r-sm);
    font-family: var(--font-mono);
    font-size: 10px;
  }
</style>
