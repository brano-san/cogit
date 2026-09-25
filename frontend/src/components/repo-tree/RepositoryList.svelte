<script lang="ts">
  import Disclosure from "$components/common/Disclosure.svelte";
  import KindIcon from "$components/common/KindIcon.svelte";
  import { applyClick, EMPTY_SELECTION, type FileSelection } from "$lib/multi-select";
  import { MISSING_REPOSITORY } from "$lib/repo-labels";
  import { canPull, rowSync, syncTooltip, type RowSync } from "$lib/repo-sync";
  import { repoPulse } from "$stores/repo-pulse.svelte";
  import {
    describeModule,
    mayExpand,
    moduleTooltip,
    splitModulePath,
    type ModuleRow,
  } from "$lib/module-tree";
  import { idleMessage, panelView } from "$lib/repo-phase";
  import { STATE_TAG_HINT, repoStateTag } from "$lib/repo-state";
  import { submodules } from "$stores/submodules.svelte";
  import { moduleForest } from "$stores/module-forest.svelte";
  import { moduleMemory } from "$stores/module-memory.svelte";
  import { UNGROUPED, groupRows, showsFilter } from "$lib/repo-groups";
  import type { RepoOverview } from "$lib/ipc";
  import { listedRepos, type ListedRepo } from "$lib/repo-list";
  import { repoList } from "$stores/repo-list.svelte";
  import { repoGroups } from "$stores/repo-groups.svelte";
  import { repository } from "$stores/repository.svelte";
  import { worktrees } from "$stores/worktrees.svelte";

  interface Props {
    /** Only the folder dialog changes the label; selecting a repository must not (R-35). */
    opening?: boolean;
    onopenmodule: (row: ModuleRow) => void;
    /** A submodule of a repository the panels do not own: open both in one click. */
    onopenforeignmodule: (root: string, row: ModuleRow) => void;
    /** `root` names the owner when it is not the repository the panels own. */
    onmodulecontext: (row: ModuleRow, x: number, y: number, root?: string) => void;
    onopen: () => void;
    onscan: () => void;
    onselect: (entry: RepoOverview) => void;
    oncontext: (row: ListedRepo, x: number, y: number) => void;
    /** A closed row was clicked: open it again. */
    onreopen: (root: string) => void;
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
    oncontext,
    onreopen,
    onmarked,
    ongroupcontext,
    onaddgroup,
    onopenmodule,
    onopenforeignmodule,
    onmodulecontext,
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
    listedRepos(repository.openRepos, repoList.list).filter((entry) =>
      `${entry.name} ${entry.root}`.toLowerCase().includes(filter.trim().toLowerCase()),
    ),
  );
  const order = $derived(entries.map((entry) => entry.root));
  const byRoot = $derived(new Map(entries.map((entry) => [entry.root, entry])));
  const rows = $derived(groupRows(repoGroups.groups, order, repoGroups.collapsed));

  const everyRoot = $derived(listedRepos(repository.openRepos, repoList.list).map((each) => each.root));

  $effect(() => repoPulse.watch(everyRoot));

  $effect(() => {
    void moduleForest.trees;
    void moduleForest.probe(order);
  });

  /** Dropping a repository that is part of a marked set moves the whole set. */
  function dropped(group: string, root: string) {
    over = null;
    const moving = marked.paths.has(root) && marked.paths.size > 1 ? [...marked.paths] : [root];
    for (const each of moving) repoGroups.assign(each, group);
  }
</script>

<!-- Push and pull sit on the corners of the icon, as SmartGit draws them; the changes dot has
     a slot of its own in every row, so the names start on one line (R-353). -->
{#snippet repoMarks(sync: RowSync)}
  {@const tip = syncTooltip(sync)}
  <span class="repo-icon">
    <KindIcon kind="repository" title={tip || undefined} />
    {#if sync.ahead > 0}
      <svg class="arrow push" viewBox="0 0 8 8" role="img" aria-label="Commits to push"
        ><path d="M4 7V1.5M1.5 4 4 1.5 6.5 4" /></svg
      >
    {/if}
    {#if sync.unknown}
      <span class="arrow unknown" role="img" aria-label="Unknown whether there is anything to pull"
        >?</span
      >
    {:else if canPull(sync)}
      <svg class="arrow pull" viewBox="0 0 8 8" role="img" aria-label="Commits to pull"
        ><path d="M4 1v5.5M1.5 4 4 6.5 6.5 4" /></svg
      >
    {/if}
  </span>
  <span
    class="changes"
    class:dirty={sync.dirty === true}
    role={sync.dirty ? "img" : undefined}
    title={sync.dirty ? tip : undefined}
    aria-label={sync.dirty ? "Uncommitted changes" : undefined}
  ></span>
{/snippet}

{#snippet topDisclosure(root: string, owned: boolean)}
  {@const open = owned ? !submodules.folded : moduleMemory.isOpen(root)}
  <Disclosure
    empty={owned ? submodules.top.length === 0 : !moduleForest.hasModules(root)}
    {open}
    label={open ? "Hide submodules" : "Show submodules"}
    onclick={(event) => {
      event.stopPropagation();
      if (owned) submodules.foldTop();
      else void moduleForest.toggleTop(root);
    }}
  />
{/snippet}

<!-- The tree of the repository the panels own is read in full; every other one is the light
     outline, and a click there opens the submodule in one go (R-352). -->
{#snippet moduleTree(root: string, owned: boolean, depth: number)}
  {@const children = owned ? submodules.children : (moduleForest.trees.get(root) ?? new Map())}
  {@const toggle = (node: ModuleRow) =>
    void (owned ? submodules.toggle(node) : moduleForest.toggle(root, node))}
  {@const open = (node: ModuleRow) => (owned ? onopenmodule(node) : onopenforeignmodule(root, node))}
  {#each owned ? submodules.rows : moduleForest.rows(root) as node (node.key)}
    {@const parts = splitModulePath(node.path)}
    {@const folder = parts.dir.replace(/[/\\]$/, "")}
    {@const where = describeModule(node.module)}
    <div
      class="row module {node.module.state}"
      class:selected={owned && submodules.open === node.key}
      role="button"
      tabindex="0"
      title="{node.path} — {node.module.url}"
      style:padding-left="calc(var(--tree-base) + {depth + 1 + node.depth} * var(--tree-step))"
      onclick={() => open(node)}
      ondblclick={() => {
        if (!owned) return;
        open(node);
        toggle(node);
      }}
      onkeydown={(event) => {
        if (event.key === "Enter") open(node);
        if (event.key === "ArrowRight" && !node.expanded) toggle(node);
        if (event.key === "ArrowLeft" && node.expanded) toggle(node);
      }}
      oncontextmenu={(event) => {
        event.preventDefault();
        onmodulecontext(node, event.clientX, event.clientY, owned ? undefined : root);
      }}
    >
      <Disclosure
        empty={!mayExpand(children, node.key, node.module)}
        open={node.expanded}
        label={node.expanded ? "Collapse" : "Expand"}
        onclick={(event) => {
          event.stopPropagation();
          toggle(node);
        }}
      />
      <KindIcon kind="submodule" />
      <span class="modname truncate shrink-last"
        >{#if folder}<span class="dir">{folder}/</span>{/if}{parts.name}</span
      >
      {#if repoStateTag(node.module.repoState, true)}
        <span class="op" title={STATE_TAG_HINT}>{repoStateTag(node.module.repoState, true)}</span>
      {/if}
      {#if where}
        <span class="where truncate shrink-first" title={moduleTooltip(node.module) || undefined}
          >({where})</span
        >
      {/if}
    </div>
  {/each}
{/snippet}

<div class="wrapper tree-rows key-list">
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

  {#if showsFilter(repository.openRepos.length + repoList.list.closed.length, filter)}
    <input
      class="filter"
      type="search"
      bind:value={filter}
      placeholder="Filter repositories"
      aria-label="Filter repositories"
    />
  {/if}

  {#if rows.length === 0}
    {@const idle = idleMessage(panelView(repository.phase))}
    {#if idle}<p class="none">{idle}</p>{/if}
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
          style:padding-left="calc(var(--tree-base) + {row.depth} * var(--tree-step))"
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
          <Disclosure open={!repoGroups.collapsed.has(row.id)} />
          <KindIcon kind="group" />
          <span class="truncate">{row.name} ({row.count})</span>
        </div>
      {:else}
        {@const listed = byRoot.get(row.root)}
        {@const entry = listed?.overview}
        {@const sync = rowSync({
          overview: entry ?? null,
          owned: entry !== undefined && entry !== null && active?.valueOf() === entry.repo.valueOf(),
          pulse: repoPulse.pulses.get(row.root),
          fetchFailed: repoPulse.unknown.has(row.root),
          remoteAhead: repoPulse.remoteAhead.has(row.root),
        })}
        {#if listed && entry}
      <div
        class="row"
        draggable="true"
        style:padding-left="calc(var(--tree-base) + {row.depth} * var(--tree-step))"
        ondragstart={(event) => event.dataTransfer?.setData("text/cogit-repo", entry.root)}
        class:selected={active?.valueOf() === entry.repo.valueOf()}
        class:holds-worktree={worktrees.ownerRoot === entry.root}
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
          oncontext(listed, event.clientX, event.clientY);
        }}
      >
        {@render topDisclosure(entry.root, submodules.owner?.valueOf() === entry.repo.valueOf())}
        {@render repoMarks(sync)}
        <span class="name truncate shrink-last">{listed.name}</span>
        {#if listed.pinned}<span class="pin" title="Pinned to the top of its group">⊤</span>{/if}
        {#if worktrees.ownerRoot === entry.root && repository.current}
          <KindIcon kind="worktree" title="The panels show its worktree {repository.current.root}" />
        {/if}
        {#if repoStateTag(entry.state)}
          <span class="op" title={STATE_TAG_HINT}>{repoStateTag(entry.state)}</span>
        {/if}
        {#if sync.missing}
          <span class="gone" title={MISSING_REPOSITORY}>missing</span>
        {/if}
        {#if entry.branch}<span class="branch truncate shrink-first">{entry.branch}</span>{/if}
      </div>

      {@render moduleTree(entry.root, submodules.owner?.valueOf() === entry.repo.valueOf(), row.depth)}
        {:else if listed}
          <div
            class="row closed"
            draggable="true"
            role="button"
            tabindex="0"
            title="{listed.root} — closed; click to open"
            style:padding-left="calc(var(--tree-base) + {row.depth} * var(--tree-step))"
            ondragstart={(event) => event.dataTransfer?.setData("text/cogit-repo", listed.root)}
            onclick={() => onreopen(listed.root)}
            onkeydown={(event) => event.key === "Enter" && onreopen(listed.root)}
            oncontextmenu={(event) => {
              event.preventDefault();
              oncontext(listed, event.clientX, event.clientY);
            }}
          >
            {@render topDisclosure(listed.root, false)}
            {@render repoMarks(sync)}
            <span class="name truncate shrink-last">{listed.name}</span>
            {#if listed.pinned}<span class="pin" title="Pinned to the top of its group">⊤</span>{/if}
            {#if sync.missing}<span class="gone" title={MISSING_REPOSITORY}>missing</span>{/if}
          </div>
          {@render moduleTree(listed.root, false, row.depth)}
        {/if}
      {/if}
    {/each}
  {/if}
</div>

<style>
  .wrapper {
    --tree-gap: var(--sp-3);
    padding: var(--sp-4) 0;
  }

  /* One run of text cut on the right like every list (R-243, which replaces R-124). */
  .dir {
    color: var(--text-secondary);
  }

  .row.module .where {
    flex-grow: 1;
    color: var(--text-secondary);
    font-size: 11px;
  }

  /* Closed reads as dimmed, text and icon alike, with no word for it (R-351). */
  .row.closed {
    color: var(--text-secondary);
  }

  .row.closed :global(.kind) {
    opacity: 0.5;
  }

  .pin {
    flex: none;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .row.holds-worktree {
    box-shadow: inset 2px 0 0 var(--status-ref);
  }

  .row.module.diverged .where,
  .row.module.behind .where,
  .row.module.notInitialised .where {
    color: var(--status-modify);
  }

  /* Ahead is work the user did and has only to record; not a warning colour. */
  .row.module.ahead .where {
    color: var(--status-add);
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
    width: var(--panel-icon);
    height: var(--panel-icon);
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
    padding: var(--sp-6) var(--sp-5);
    text-align: center;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }


  .row {
    display: flex;
    align-items: center;
    gap: var(--tree-gap);
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
    gap: var(--tree-gap);
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

  .op {
    flex: 0 0 auto;
    color: var(--status-modify);
    font-size: 10px;
  }

  .repo-icon {
    position: relative;
    display: inline-flex;
    flex: none;
  }

  /* On the corners, with a halo of the panel colour, so they sit on the icon's edge
     without covering it. */
  .arrow {
    position: absolute;
    right: -3px;
    width: 7px;
    height: 7px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
    filter: drop-shadow(0 0 1px var(--surface-panel));
  }

  .arrow.push {
    top: -2px;
    color: var(--indicator-push);
  }

  .arrow.pull {
    bottom: -2px;
    color: var(--indicator-pull);
  }

  .arrow.unknown {
    bottom: -3px;
    width: auto;
    height: auto;
    color: var(--indicator-unknown);
    font-size: 8px;
    font-weight: 700;
    line-height: 1;
  }

  .changes {
    flex: 0 0 6px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }

  .changes.dirty {
    background: var(--indicator-changes);
  }

  .branch {
    flex-grow: 1;
    color: var(--text-secondary);
    font-size: 10px;
  }

</style>
