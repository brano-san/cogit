<script lang="ts">
  import { describeModule, splitModulePath, type ModuleRow } from "$lib/module-tree";
  import { submodules } from "$stores/submodules.svelte";

  /** Submodules as branches of the repository that owns them, to any depth, expanded one
      node at a time. Double-click opens one; it stays a node here rather than becoming a
      second entry in the list (doc/12-risks.md, R-109, R-110). */
  interface Props {
    /** Opens the submodule in the panels. */
    onopen: (row: ModuleRow) => void;
    /** `git submodule update`, with `--init` when it has never been checked out. */
    onupdate: (row: ModuleRow) => void;
    oncontext: (row: ModuleRow, x: number, y: number) => void;
  }

  let { onopen, onupdate, oncontext }: Props = $props();

  const rows = $derived(submodules.rows);

  function activate(row: ModuleRow) {
    if (row.module.state === "notInitialised") onupdate(row);
    else onopen(row);
  }
</script>

{#if rows.length > 0}
  <div class="section" role="tree" aria-label="Submodules">
    <div class="section-header">Submodules</div>
    {#each rows as row (row.key)}
      {@const parts = splitModulePath(row.path)}
      <div
        class="row {row.module.state}"
        class:open={submodules.open === row.key}
        role="treeitem"
        tabindex="-1"
        aria-level={row.depth + 1}
        aria-expanded={row.expanded}
        aria-selected={submodules.open === row.key}
        title="{row.module.url} — recorded {row.module.recorded}"
        style:padding-left="calc(var(--sp-5) + {row.depth * 14}px)"
        ondblclick={() => activate(row)}
        oncontextmenu={(event) => {
          event.preventDefault();
          oncontext(row, event.clientX, event.clientY);
        }}
        onkeydown={(event) => {
          if (event.key === "Enter") activate(row);
          if (event.key === "ArrowRight" && !row.expanded) void submodules.toggle(row);
          if (event.key === "ArrowLeft" && row.expanded) void submodules.toggle(row);
        }}
      >
        <button
          type="button"
          class="caret"
          aria-label={row.expanded ? "Collapse" : "Expand"}
          onclick={(event) => {
            event.stopPropagation();
            void submodules.toggle(row);
          }}
        >
          {row.expanded ? "▾" : "▸"}
        </button>
        <span class="name truncate">
          {#if parts.dir}<span class="dir">{parts.dir}</span>{/if}{parts.name}
        </span>
        <span class="where truncate">({describeModule(row.module)})</span>
        {#if row.module.state !== "inSync"}
          <button
            type="button"
            class="act"
            title={row.module.state === "notInitialised"
              ? "Check it out for the first time"
              : "Check out the commit the parent records"}
            onclick={(event) => {
              event.stopPropagation();
              onupdate(row);
            }}
          >
            {row.module.state === "notInitialised" ? "Init" : "Update"}
          </button>
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
    padding: var(--sp-3) var(--sp-5) var(--sp-2, 3px);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-2, 3px);
    height: var(--h-row-dense);
    padding-right: var(--sp-5);
    font-size: var(--fs-dense);
    white-space: nowrap;
  }

  .row:hover {
    background: var(--state-hover);
  }

  /* The open submodule is the one the panels are showing, so it is marked the way a
     selected repository is rather than with an icon of its own. */
  .row.open {
    background: var(--state-selected);
    box-shadow: inset 2px 0 0 var(--status-ref);
    font-weight: 600;
  }

  .caret {
    flex: 0 0 auto;
    width: 14px;
    padding: 0;
    background: none;
    border: 0;
    color: var(--text-secondary);
    font: inherit;
    font-size: 9px;
    cursor: default;
  }

  .caret:hover {
    color: var(--text-primary);
  }

  .name {
    flex: 0 1 auto;
    min-width: 0;
  }

  /* The folder is context, the name is the thing: SmartGit dims the first. */
  .dir {
    color: var(--text-secondary);
  }

  .where {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .row.diverged .where {
    color: var(--status-modify);
  }

  .act {
    flex: 0 0 auto;
    padding: 0 var(--sp-3);
    background: none;
    border: 0;
    color: var(--text-secondary);
    font: inherit;
    font-size: 10px;
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
