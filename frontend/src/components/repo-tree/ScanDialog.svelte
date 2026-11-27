<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import Icon from "$components/common/Icon.svelte";
  import { scanFilter, scanRows, scanSections, type ScanLayout } from "$lib/scan-groups";
  import { scan } from "$stores/scan.svelte";
  import { settings } from "$stores/settings.svelte";
  import ScanResults from "./ScanResults.svelte";

  interface Props {
    busy: boolean;
    onbrowse: () => void;
    onopen: (roots: string[]) => void;
    onclose: () => void;
  }

  let { busy, onbrowse, onopen, onclose }: Props = $props();

  let input: HTMLInputElement | undefined = $state();
  let filter = $state("");
  let layout = $state<ScanLayout>("folders");
  let expansion = $state.raw<ReadonlyMap<string, boolean>>(new Map());
  let searchExpansion = $state.raw<ReadonlyMap<string, boolean>>(new Map());

  const search = $derived(scanFilter(filter, settings.current.filterCase));
  const sections = $derived(scanSections(scan.hits, layout));
  const shown = $derived(search.active
    ? sections.map((section) => ({ ...section, hits: section.hits.filter(search.matches) }))
      .filter((section) => section.hits.length > 0)
    : sections);
  const matches = $derived(shown.reduce((sum, section) => sum + section.hits.length, 0));
  const rows = $derived(scanRows(shown, search.active ? searchExpansion : expansion, search.active));
  const scope = $derived(shown.flatMap((section) =>
    section.hits.filter((hit) => !hit.alreadyOpen).map((hit) => hit.root)));
  const allSelected = $derived(scope.length > 0 && scope.every((root) => scan.chosen.has(root)));
  const chosen = $derived(scan.selected);

  function setFilter(value: string) {
    if (value.trim() !== filter.trim()) searchExpansion = new Map();
    filter = value;
  }

  function clearFilter() {
    setFilter("");
    input?.focus();
  }

  function expand(group: string, expanded: boolean) {
    if (search.active) searchExpansion = new Map(searchExpansion).set(group, expanded);
    else expansion = new Map(expansion).set(group, expanded);
  }

  function open() {
    if (chosen.length > 0 && !busy) onopen(chosen);
  }
</script>

<Dialog title="Search for repositories" width="720px" height="min(520px, 90vh)" flush {onclose} onconfirm={open}>
  <div class="toolbar">
    <div class="location">
      <button class="btn" type="button" onclick={onbrowse} disabled={busy}>
        <Icon name="folder" />
        Choose Folder…
      </button>
      <span class="location-path truncate" data-tip={scan.folder ?? undefined}>
        {scan.folder ?? "No folder chosen"}
      </span>
    </div>

    <div class="controls">
      <div class="search">
        <input
          type="search"
          value={filter}
          placeholder="Search repositories…"
          aria-label="Search repositories"
          data-autofocus
          bind:this={input}
          oninput={(event) => setFilter(event.currentTarget.value)}
          onkeydown={(event) => {
            if (event.key !== "Escape" || !filter || event.isComposing) return;
            event.preventDefault();
            clearFilter();
          }}
        />
        {#if filter}
          <button type="button" class="clear-search" aria-label="Clear search" data-tip="Clear search (Esc)" onclick={clearFilter}>
            <Icon name="close" />
          </button>
        {/if}
      </div>
      <div class="actions">
        <div class="view-switch" role="group" aria-label="Results view">
          <button type="button" class="btn" aria-pressed={layout === "folders"} onclick={() => (layout = "folders")}>
            <Icon name="directories" /> Tree
          </button>
          <button type="button" class="btn" aria-pressed={layout === "flat"} onclick={() => (layout = "flat")}>
            <Icon name="flatList" /> List
          </button>
        </div>
        <button class="btn select-all" type="button" onclick={() => scan.toggleAll(scope)} disabled={scope.length === 0}>
          {allSelected ? "Select None" : "Select All"}
        </button>
      </div>
    </div>
  </div>

  {#if scan.error}
    <div class="scan-error" class:standalone={rows.length === 0} role="alert">
      <Icon name="error" />
      <div>
        <strong>Could not complete the scan</strong>
        <div class="error-detail">{scan.error.message}</div>
      </div>
    </div>
  {/if}

  {#if rows.length > 0}
    <ScanResults {rows} {layout} {search} {scope} onexpand={expand} />
  {:else if !scan.error}
    <div class="empty" role="status">
      {#if scan.hits.length > 0}
        <p>No repositories match your search.</p>
        <button type="button" class="btn" onclick={clearFilter}>Clear search</button>
      {:else if scan.busy}
        <p>Scanning for repositories…</p>
      {:else if scan.stopped}
        <p>The scan stopped before finding any repositories.</p>
      {:else if scan.done}
        <p>No repositories found in this location.</p>
      {:else}
        <p>Choose a folder to search for repositories.</p>
      {/if}
    </div>
  {/if}

  {#snippet footer()}
    <div class="summary" aria-live="polite" aria-atomic="true">
      {#if scan.busy}<span>Scanning…</span>{/if}
      {#if scan.stopped}<span>Scan stopped</span>{/if}
      <span>{scan.hits.length} found</span>
      {#if search.active}<span>{matches} matching</span>{/if}
      <span>{chosen.length} selected</span>
    </div>
    {#if chosen.length > 0}
      <button type="button" class="btn quiet" onclick={() => {
        scan.clearSelection();
        input?.focus();
      }}>Clear selection</button>
    {/if}
    {#if scan.busy}
      <button type="button" class="btn" onclick={() => {
        scan.stop();
        input?.focus();
      }}>Stop scan</button>
    {/if}
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" disabled={chosen.length === 0 || busy} onclick={open}>
      {busy ? "Opening…" : "Open (" + chosen.length + ")"}
    </button>
  {/snippet}
</Dialog>

<style>
  .toolbar {
    flex: 0 0 auto;
    display: grid;
    gap: var(--sp-4);
    padding: var(--sp-4) var(--sp-5);
    border-bottom: 1px solid var(--divider);
  }

  .location,
  .controls,
  .actions {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    min-width: 0;
  }

  .location-path {
    flex: 1 1 auto;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .controls,
  .actions {
    flex-wrap: wrap;
  }

  .search {
    position: relative;
    flex: 1 1 12rem;
    min-width: 0;
  }

  .search input {
    display: block;
  }

  .search input::-webkit-search-cancel-button {
    display: none;
  }

  .search:has(.clear-search) input {
    padding-inline-end: calc(var(--h-button-sm) + var(--sp-4));
  }

  .clear-search {
    position: absolute;
    inset-inline-end: var(--sp-2);
    top: 50%;
    transform: translateY(-50%);
    display: grid;
    place-content: center;
    width: var(--h-button-sm);
    height: var(--h-button-sm);
    color: var(--text-secondary);
    border-radius: var(--r-sm);
  }

  .clear-search:hover {
    background: var(--state-hover);
    color: var(--text-primary);
  }

  .view-switch {
    display: flex;
  }

  .view-switch .btn:first-child {
    border-start-end-radius: 0;
    border-end-end-radius: 0;
  }

  .view-switch .btn:last-child {
    border-start-start-radius: 0;
    border-end-start-radius: 0;
    margin-inline-start: -1px;
  }

  .view-switch .btn:is([aria-pressed="true"], :hover, :focus-visible) {
    position: relative;
  }

  .view-switch .btn:focus-visible {
    z-index: 1;
  }

  .select-all {
    min-width: 12ch;
  }

  .empty,
  .scan-error {
    font-size: var(--fs-dense);
    background: var(--surface-base);
  }

  .empty,
  .scan-error.standalone {
    flex: 1 1 auto;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .empty {
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-5);
    text-align: center;
    color: var(--text-secondary);
  }

  .empty p {
    margin: 0;
  }

  .scan-error {
    display: flex;
    flex-shrink: 0;
    align-items: baseline;
    gap: var(--sp-3);
    max-height: 40%;
    overflow: auto;
    padding: var(--sp-4) var(--sp-5);
    color: var(--status-delete);
  }

  .scan-error.standalone {
    max-height: none;
    align-items: flex-start;
    align-content: center;
    flex-wrap: wrap;
  }

  .error-detail {
    color: var(--text-primary);
    user-select: text;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .summary {
    flex: 1 1 auto;
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2) var(--sp-4);
    color: var(--text-secondary);
    font-size: var(--fs-status);
    font-variant-numeric: tabular-nums;
  }

  .quiet {
    background: transparent;
    border-color: transparent;
  }
</style>
