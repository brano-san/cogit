<script lang="ts">
    import { applyClick, EMPTY_SELECTION, type FileSelection } from "$lib/multi-select";
  import { panelView } from "$lib/repo-phase";
  import { UNGROUPED, groupRows } from "$lib/repo-groups";
  import type { RepoOverview } from "$lib/ipc";
  import { repoGroups } from "$stores/repo-groups.svelte";
  import { repository } from "$stores/repository.svelte";

  interface Props {
    /** Only the folder dialog changes the label; selecting a repository must not (R-35). */
    opening?: boolean;
    onopen: () => void;
    onscan: () => void;
    onselect: (entry: RepoOverview) => void;
    onclose: (entry: RepoOverview) => void;
    oncontext: (entry: RepoOverview, x: number, y: number) => void;
    /** Ticked rows, for actions that work on several repositories at once (T3.3). */
    onmarked: (roots: string[]) => void;
    /** Renaming and deleting a group live in the caller's dialogs, not here. */
    ongroupcontext: (id: string, x: number, y: number) => void;
    onaddgroup: () => void;
  }

  let {
    opening = false,
    onopen,
    onscan,
    onselect,
    onclose,
    oncontext,
    onmarked,
    ongroupcontext,
    onaddgroup,
  }: Props = $props();

  /** The group a drag is hovering, so the drop target is visible before the drop. */
  let over = $state<string | null>(null);

  let filter = $state("");
  let marked = $state.raw<FileSelection>(EMPTY_SELECTION);

  $effect(() => {
    onmarked([...marked.paths]);
  });

  const active = $derived(repository.current?.repo);
  const entries = $derived(
    repository.openRepos.filter((entry) =>
      `${entry.name} ${entry.root}`.toLowerCase().includes(filter.trim().toLowerCase()),
    ),
  );
  const order = $derived(entries.map((entry) => entry.root));
  const byRoot = $derived(new Map(entries.map((entry) => [entry.root, entry])));
  const rows = $derived(groupRows(repoGroups.groups, order, repoGroups.collapsed));

  /** Dropping a repository that is part of a marked set moves the whole set. */
  function dropped(group: string, root: string) {
    over = null;
    const moving = marked.paths.has(root) && marked.paths.size > 1 ? [...marked.paths] : [root];
    for (const each of moving) repoGroups.assign(each, group);
  }
</script>

<div class="wrapper">
  <div class="actions" role="toolbar" aria-label="Repository list actions">
    <button
      type="button"
      class="tool"
      onclick={onopen}
      disabled={repository.busy}
      title={opening ? "Opening…" : "Open Repository…"}
      aria-label="Open Repository"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true"
        ><path
          d="M4 5.5A1.5 1.5 0 0 1 5.5 4h3.2a1.5 1.5 0 0 1 1.2.6l1 1.4h7.6A1.5 1.5 0 0 1 20 7.5v11A1.5 1.5 0 0 1 18.5 20h-13A1.5 1.5 0 0 1 4 18.5Z"
        /></svg
      >
    </button>
    <button
      type="button"
      class="tool"
      onclick={onscan}
      disabled={repository.busy}
      title="Scan Folder for Repositories…"
      aria-label="Scan folder for repositories"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true"
        ><circle cx="11" cy="11" r="6" /><path d="m20 20-4.3-4.3" /></svg
      >
    </button>
    <button
      type="button"
      class="tool"
      onclick={onaddgroup}
      title="New Group"
      aria-label="New group"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true"
        ><path
          d="M4 6.5A1.5 1.5 0 0 1 5.5 5h3.2a1.5 1.5 0 0 1 1.2.6l.9 1.4h7.7A1.5 1.5 0 0 1 20 8.5v9A1.5 1.5 0 0 1 18.5 19h-13A1.5 1.5 0 0 1 4 17.5Z"
        /><path d="M12 10.5v5M9.5 13h5" /></svg
      >
    </button>
  </div>

  {#if repository.openRepos.length > 1}
    <input
      class="filter"
      type="search"
      bind:value={filter}
      placeholder="Filter repositories"
      aria-label="Filter repositories"
    />
  {/if}

  {#if rows.length === 0}
    <p class="none">
      {panelView(repository.phase) === "opening" ? "Opening repository…" : "No repository open."}
    </p>
  {:else}
    {#each rows as row (row.kind === "group" ? `g:${row.id}` : row.root)}
      {#if row.kind === "group"}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="group"
          class:over={over === row.id}
          role="button"
          tabindex="0"
          draggable={row.id !== UNGROUPED}
          style:padding-left="calc(var(--sp-4) + {row.depth * 12}px)"
          ondragstart={(event) => event.dataTransfer?.setData("text/cogit-group", row.id)}
          onclick={() => repoGroups.collapse(row.id)}
          onkeydown={(event) => event.key === "Enter" && repoGroups.collapse(row.id)}
          oncontextmenu={(event) => {
            if (row.id === UNGROUPED) return;
            event.preventDefault();
            ongroupcontext(row.id, event.clientX, event.clientY);
          }}
          ondragover={(event) => {
            event.preventDefault();
            over = row.id;
          }}
          ondragleave={() => (over = null)}
          ondrop={(event) => {
            over = null;
            const moved = event.dataTransfer?.getData("text/cogit-group") ?? "";
            if (moved) {
              repoGroups.nest(moved, row.id === UNGROUPED ? null : row.id);
              return;
            }
            const root = event.dataTransfer?.getData("text/cogit-repo") ?? "";
            if (root) dropped(row.id, root);
          }}
        >
          <span class="caret" aria-hidden="true"
            >{repoGroups.collapsed.has(row.id) ? "▸" : "▾"}</span
          >
          <span class="truncate">{row.name} ({row.count})</span>
        </div>
      {:else}
        {@const entry = byRoot.get(row.root)}
        {#if entry}
      <div
        class="row"
        draggable="true"
        style:padding-left="calc(var(--sp-5) + {row.depth * 12}px)"
        ondragstart={(event) => event.dataTransfer?.setData("text/cogit-repo", entry.root)}
        class:selected={active?.valueOf() === entry.repo.valueOf()}
        class:marked={marked.paths.has(entry.root)}
        class:missing={entry.missing}
        role="button"
        tabindex="0"
        title={entry.root}
        onclick={(event) => {
          marked = applyClick(marked, entry.root, order, {
            ctrl: event.ctrlKey || event.metaKey,
            shift: event.shiftKey,
          });
          if (!event.ctrlKey && !event.metaKey && !event.shiftKey) onselect(entry);
        }}
        onkeydown={(event) => event.key === "Enter" && onselect(entry)}
        oncontextmenu={(event) => {
          event.preventDefault();
          oncontext(entry, event.clientX, event.clientY);
        }}
      >
        <svg class="folder" viewBox="0 0 16 16" aria-hidden="true"
          ><path
            fill="currentColor"
            d="M1.5 3.5c0-.69.56-1.25 1.25-1.25h3.04c.4 0 .78.19 1.01.51l.79 1.09h5.66c.69 0 1.25.56 1.25 1.25v7.15c0 .69-.56 1.25-1.25 1.25H2.75c-.69 0-1.25-.56-1.25-1.25V3.5Z"
          /></svg
        >
        <span class="name truncate">{entry.name}</span>
        {#if entry.missing}
          <span class="gone" title="This folder is no longer on disk">missing</span>
        {:else if entry.dirty}
          <span class="dirty" title="Uncommitted changes">●</span>
        {/if}
        {#if entry.branch}<span class="branch truncate">{entry.branch}</span>{/if}
        {#if entry.ahead > 0 || entry.behind > 0}
          <span class="track tabular"
            >{entry.ahead > 0 ? "↑" + entry.ahead : ""}{entry.behind > 0
              ? "↓" + entry.behind
              : ""}</span
          >
        {/if}
        <span
          class="act"
          role="button"
          tabindex="-1"
          title="Close {entry.name}"
          onclick={(event) => {
            event.stopPropagation();
            onclose(entry);
          }}
          onkeydown={(event) => event.key === "Enter" && onclose(entry)}>✕</span
        >
      </div>
        {/if}
      {/if}
    {/each}
  {/if}
</div>

<style>
  .wrapper {
    padding: var(--sp-4) 0;
  }

  .actions {
    display: flex;
    gap: var(--sp-1);
    margin: 0 var(--sp-3) var(--sp-2);
  }

  .tool {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 24px;
    padding: 0;
    background: none;
    border: 0;
    border-radius: var(--r-sm);
    color: var(--text-secondary);
    cursor: default;
  }

  .tool svg {
    width: 15px;
    height: 15px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .tool:not(:disabled):hover {
    background: var(--state-hover);
    color: var(--text-primary);
  }

  .tool:disabled {
    opacity: 0.4;
  }

  /* One line, the same shape every other empty panel uses. */
  .none {
    margin: 0;
    padding: var(--sp-6, 16px) var(--sp-5);
    text-align: center;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
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

  /* A bar as well as a tint: two greys apart is not something everyone can see. */
  .row.selected {
    background: var(--state-selected);
    box-shadow: inset 2px 0 0 var(--status-ref);
  }

  .group {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    height: var(--h-row-dense);
    padding: 0 var(--sp-5);
    background: var(--surface-raised);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    white-space: nowrap;
  }

  .group.over {
    box-shadow: inset 0 0 0 1px var(--status-ref);
  }

  .caret {
    flex: 0 0 auto;
    font-size: 9px;
  }

  .row.marked {
    box-shadow: inset 2px 0 0 var(--status-ref);
  }

  .row.missing .name {
    color: var(--text-secondary);
    text-decoration: line-through;
  }

  .gone {
    flex: 0 0 auto;
    color: var(--status-delete);
    font-size: 10px;
    letter-spacing: 0.04em;
  }

  .filter {
    display: block;
    width: calc(100% - var(--sp-5) * 2);
    margin: 0 var(--sp-5) var(--sp-4);
    height: var(--h-button-sm);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
  }

  .folder {
    flex: 0 0 auto;
    width: 13px;
    height: 13px;
    color: var(--status-ref);
  }

  .name {
    flex: 0 1 auto;
    min-width: 0;
  }

  .dirty {
    flex: 0 0 auto;
    color: var(--status-modify);
    font-size: 9px;
  }

  .branch {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--text-secondary);
    font-size: 10px;
  }

  .track {
    flex: 0 0 auto;
    color: var(--status-ref);
    font-size: 10px;
  }

  .act {
    flex: 0 0 auto;
    padding: 0 var(--sp-2, 3px);
    color: var(--text-secondary);
    opacity: 0;
    cursor: default;
  }

  .row:hover .act {
    opacity: 1;
  }

  .act:hover {
    color: var(--status-delete);
  }
</style>
