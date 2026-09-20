<script lang="ts">
  import { buildTree, matchesFilter } from "$lib/ref-tree";
  import { DRAG_TYPE, parseDrag, serialiseDrag } from "$lib/drop-target";
  import type { Branch } from "$lib/ipc";

  interface Props {
    title: string;
    branches: Branch[];
    oncheckout?: (branch: Branch) => void;
    ondelete?: (branch: Branch) => void;
    onmerge?: (branch: Branch) => void;
    onrebase?: (branch: Branch) => void;
    /** A branch was dropped on another branch; the caller offers the choices. */
    ondrop?: (source: string, target: Branch) => void;
    /** Typed in the panel header; folders whose children all fail it disappear with them. */
    filter?: string;
  }

  let {
    title,
    branches,
    oncheckout,
    ondelete,
    onmerge,
    onrebase,
    ondrop,
    filter = "",
  }: Props = $props();

  let over = $state<string | null>(null);

  function dropped(event: DragEvent, target: Branch) {
    over = null;
    const text = event.dataTransfer?.getData(DRAG_TYPE) ?? "";
    const payload = parseDrag(text);
    if (payload?.kind === "branch" && payload.id !== target.name) ondrop?.(payload.id, target);
  }

  let collapsed = $state(false);
  const shown = $derived(branches.filter((b) => matchesFilter(b.name, filter)));
  const rows = $derived(buildTree(shown));
</script>

{#if shown.length > 0}
  <div class="section">
    <div
      class="section-header"
      role="button"
      tabindex="0"
      onclick={() => (collapsed = !collapsed)}
      onkeydown={(event) => event.key === "Enter" && (collapsed = !collapsed)}
    >
      <span class="caret" aria-hidden="true">{collapsed ? "▸" : "▾"}</span>
      {title} ({shown.length})
    </div>
    {#each collapsed ? [] : rows as row (row.branch?.fullName ?? row.label + row.depth)}
      {#if !row.branch}
        <div class="folder" style:padding-left="calc(var(--sp-5) + {row.depth * 12}px)">
          {row.label}
        </div>
      {:else}
        {@const branch = row.branch}
        <div
          class="row"
          class:head={branch.isHead}
          class:over={over === branch.name}
          title={branch.fullName}
          style:padding-left="calc(var(--sp-5) + {row.depth * 12}px)"
          role="listitem"
          draggable={ondrop !== undefined}
          ondragstart={(event) =>
            event.dataTransfer?.setData(DRAG_TYPE, serialiseDrag({ kind: "branch", id: branch.name }))}
          ondragover={(event) => {
            if (ondrop) {
              event.preventDefault();
              over = branch.name;
            }
          }}
          ondragleave={() => (over = null)}
          ondrop={(event) => dropped(event, branch)}
        >
        <span class="marker" aria-hidden="true">{branch.isHead ? "▸" : ""}</span>
        <span class="name truncate">{row.label}</span>
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
      {/if}
    {/each}
  </div>
{/if}

<style>
  .section {
    padding-bottom: var(--sp-4);
  }

  .caret {
    display: inline-block;
    width: 10px;
    color: var(--text-secondary);
  }

  .folder {
    display: flex;
    align-items: center;
    height: 22px;
    padding-right: var(--sp-5);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
    white-space: nowrap;
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

  .row.over {
    box-shadow: inset 0 0 0 1px var(--status-ref);
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
