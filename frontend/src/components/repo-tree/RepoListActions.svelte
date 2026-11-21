<script lang="ts">
  import Icon from "$components/common/Icon.svelte";
  import { showsFilter } from "$lib/repo-groups";
  import { repoList } from "$stores/repo-list.svelte";
  import { repository } from "$stores/repository.svelte";

  interface Props {
    /** True only while the folder dialog's own flow runs, so the label does not flicker. */
    opening: boolean;
    filter: string;
    onopen: () => void;
    onremote: () => void;
    onscan: () => void;
    onaddgroup: () => void;
  }

  let { opening, filter = $bindable(), onopen, onremote, onscan, onaddgroup }: Props = $props();
</script>

<!-- In the panel's header, as SmartGit keeps it: a toolbar row of its own cost the list a row (R-792). -->
{#if showsFilter(repository.openRepos.length + repoList.list.closed.length, filter)}
  <input
    class="filter"
    type="search"
    bind:value={filter}
    placeholder="Filter repositories"
    aria-label="Filter repositories"
  />
{/if}
<button
  type="button"
  class="tool"
  onclick={onopen}
  disabled={repository.busy}
  title={opening ? "Opening…" : "Open Repository…"}
  aria-label="Open Repository"
>
  <Icon name="folder" />
</button>
<button type="button" class="tool" onclick={onremote} disabled={repository.busy} title="Add Remote Repository…" aria-label="Add Remote Repository">
  <Icon name="repository" />
</button>
<button
  type="button"
  class="tool"
  onclick={onscan}
  disabled={repository.busy}
  title="Search for Repositories…"
  aria-label="Search for repositories"
>
  <!-- A magnifier over a repository: a bare one read as the panel's own filter. -->
  <Icon name="scanRepositories" />
</button>
<button type="button" class="tool" onclick={onaddgroup} title="New Group" aria-label="New group">
  <Icon name="folderPlus" />
</button>

<style>
  .tool {
    display: inline-flex;
    flex: 0 0 auto;
    align-items: center;
    justify-content: center;
    width: var(--h-button-sm);
    height: var(--h-button-sm);
    padding: 0;
    background: none;
    border: 0;
    border-radius: var(--r-sm);
    color: var(--text-secondary);
    cursor: default;
  }

  .tool :global(svg) {
    width: var(--panel-icon);
    height: var(--panel-icon);
    fill: none;
    stroke: currentColor;
    stroke-width: 1.2;
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

  /* The Branches header's filter field, so the two headers read alike. */
  .filter {
    flex: 0 1 var(--w-panel-filter);
    width: var(--w-panel-filter);
    min-width: var(--w-panel-filter-min);
    height: var(--h-panel-field);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
  }
</style>
