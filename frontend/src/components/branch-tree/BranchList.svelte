<script lang="ts">
  import type { Branch } from "$lib/ipc";

  interface Props {
    title: string;
    branches: Branch[];
    oncheckout?: (branch: Branch) => void;
    ondelete?: (branch: Branch) => void;
    onmerge?: (branch: Branch) => void;
    onrebase?: (branch: Branch) => void;
  }

  let { title, branches, oncheckout, ondelete, onmerge, onrebase }: Props = $props();
</script>

{#if branches.length > 0}
  <div class="section">
    <div class="section-header">{title} ({branches.length})</div>
    {#each branches as branch (branch.fullName)}
      <div class="row" class:head={branch.isHead} title={branch.fullName}>
        <span class="marker" aria-hidden="true">{branch.isHead ? "▸" : ""}</span>
        <span class="name truncate">{branch.name}</span>
        {#if branch.ahead > 0 || branch.behind > 0}
          <span class="track tabular" title="{branch.ahead} ahead, {branch.behind} behind">
            {branch.ahead > 0 ? "↑" + branch.ahead : ""}{branch.behind > 0
              ? "↓" + branch.behind
              : ""}
          </span>
        {:else if branch.upstream}
          <span class="track" title="In step with {branch.upstream}">=</span>
        {/if}
        <span class="oid mono tabular">{branch.oid.slice(0, 7)}</span>
        {#if !branch.isHead}
          <span
            class="act"
            role="button"
            tabindex="-1"
            title="Check out {branch.name}"
            onclick={() => oncheckout?.(branch)}
            onkeydown={(event) => event.key === "Enter" && oncheckout?.(branch)}>Checkout</span
          >
          {#if onmerge}
            <span
              class="act"
              role="button"
              tabindex="-1"
              title="Merge {branch.name} into the current branch"
              onclick={() => onmerge?.(branch)}
              onkeydown={(event) => event.key === "Enter" && onmerge?.(branch)}>Merge</span
            >
          {/if}
          {#if onrebase}
            <span
              class="act"
              role="button"
              tabindex="-1"
              title="Rebase the current branch onto {branch.name}"
              onclick={() => onrebase?.(branch)}
              onkeydown={(event) => event.key === "Enter" && onrebase?.(branch)}>Rebase</span
            >
          {/if}
          {#if branch.kind === "local"}
            <span
              class="act"
              role="button"
              tabindex="-1"
              title="Delete {branch.name}"
              onclick={() => ondelete?.(branch)}
              onkeydown={(event) => event.key === "Enter" && ondelete?.(branch)}>Delete</span
            >
          {/if}
        {/if}
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

  .track {
    flex: 0 0 auto;
    color: var(--status-ref);
    font-size: 10px;
  }

  .oid {
    color: var(--text-secondary);
    font-size: 11px;
  }
</style>
