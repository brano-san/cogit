<script lang="ts">
  import { showsFilter } from "$lib/repo-groups";
  import { repoList } from "$stores/repo-list.svelte";
  import { repository } from "$stores/repository.svelte";

  interface Props {
    /** True only while the folder dialog's own flow runs, so the label does not flicker. */
    opening: boolean;
    filter: string;
    onopen: () => void;
    onscan: () => void;
    onaddgroup: () => void;
  }

  let { opening, filter = $bindable(), onopen, onscan, onaddgroup }: Props = $props();
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
  title="Search for Repositories…"
  aria-label="Search for repositories"
>
  <!-- A magnifier over a repository: a bare one read as the panel's own filter. -->
  <svg viewBox="0 0 24 24" aria-hidden="true"
    ><clipPath id="scan-repo-cut"
      ><path clip-rule="evenodd" d="M0 0h24v24H0ZM15.5 9.7a5.8 5.8 0 1 0 0 11.6 5.8 5.8 0 1 0 0-11.6Z" /></clipPath
    ><g clip-path="url(#scan-repo-cut)"
      ><rect x="3.5" y="3" width="11" height="13" rx="1.8" /><path d="M3.5 7.3h11M3.5 11.7h11" /></g
    ><circle cx="15.5" cy="15.5" r="4" /><path d="m18.4 18.4 2.6 2.6" /></svg
  >
</button>
<button type="button" class="tool" onclick={onaddgroup} title="New Group" aria-label="New group">
  <svg viewBox="0 0 24 24" aria-hidden="true"
    ><path
      d="M4 6.5A1.5 1.5 0 0 1 5.5 5h3.2a1.5 1.5 0 0 1 1.2.6l.9 1.4h7.7A1.5 1.5 0 0 1 20 8.5v9A1.5 1.5 0 0 1 18.5 19h-13A1.5 1.5 0 0 1 4 17.5Z"
    /><path d="M12 10.5v5M9.5 13h5" /></svg
  >
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

  /* The Branches header's filter field, so the two headers read alike. */
  .filter {
    flex: 0 1 var(--w-panel-filter);
    width: var(--w-panel-filter);
    min-width: var(--w-panel-filter-min);
    height: var(--h-button-sm);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
  }
</style>
