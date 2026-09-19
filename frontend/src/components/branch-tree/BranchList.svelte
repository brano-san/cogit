<script lang="ts">
  import type { Branch } from "$lib/ipc";

  interface Props {
    title: string;
    branches: Branch[];
  }

  let { title, branches }: Props = $props();
</script>

{#if branches.length > 0}
  <div class="section">
    <div class="section-header">{title} ({branches.length})</div>
    {#each branches as branch (branch.fullName)}
      <div class="row" class:head={branch.isHead} title={branch.fullName}>
        <span class="marker" aria-hidden="true">{branch.isHead ? "▸" : ""}</span>
        <span class="name truncate">{branch.name}</span>
        <span class="oid mono tabular">{branch.oid.slice(0, 7)}</span>
      </div>
    {/each}
  </div>
{/if}

<style>
  .section {
    padding-bottom: var(--sp-4);
  }

  .section-header {
    padding: var(--sp-3) var(--sp-5) var(--sp-2);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-secondary);
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: var(--h-row);
    padding: 0 var(--sp-5);
    font-size: var(--fs-dense);
    position: relative;
    cursor: default;
    transition: background var(--t-fast) var(--ease-out);
  }

  .row:hover {
    background: var(--state-hover);
  }

  /* The checked-out branch is marked by a bar, not by colour alone: the difference
     between the hover and selected backgrounds is too small to carry meaning. */
  .row.head {
    background: var(--state-selected);
  }

  .row.head::before {
    content: "";
    position: absolute;
    inset-block: 0;
    inset-inline-start: 0;
    width: 2px;
    background: var(--status-ref);
  }

  .marker {
    width: 8px;
    color: var(--status-ref);
    font-size: 9px;
  }

  .name {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--status-ref);
  }

  .oid {
    color: var(--text-secondary);
    font-size: 11px;
  }
</style>
