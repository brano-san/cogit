<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import { scan } from "$stores/scan.svelte";

  interface Props {
    busy: boolean;
    onbrowse: () => void;
    onopen: (roots: string[]) => void;
    onclose: () => void;
  }

  let { busy, onbrowse, onopen, onclose }: Props = $props();

  let filter = $state("");

  const shown = $derived(
    scan.hits.filter(
      (hit) =>
        filter.trim() === "" ||
        `${hit.name} ${hit.root}`.toLowerCase().includes(filter.trim().toLowerCase()),
    ),
  );
  const chosen = $derived(scan.selected);

  /** "Open 0 Selected" reads like a broken button; with nothing ticked the button says
      what it is waiting for instead. */
  const openLabel = $derived(
    busy
      ? "Opening…"
      : chosen.length === 0
        ? "Open Selected"
        : chosen.length === 1
          ? "Open 1 Repository"
          : `Open ${chosen.length} Repositories`,
  );

  function open() {
    if (chosen.length > 0 && !busy) onopen(chosen);
  }
</script>

<Dialog
  title="Scan folder for repositories"
  width="min(720px, 92vw)"
  {onclose}
  onconfirm={open}
>
  <div class="bar">
    <button type="button" onclick={onbrowse} disabled={scan.busy}>Choose Folder…</button>
    <span class="folder truncate" title={scan.folder ?? ""}>{scan.folder ?? "No folder chosen"}</span>
  </div>

  {#if scan.folder}
    <div class="bar">
      <input
        type="search"
        bind:value={filter}
        placeholder="Filter by name or path"
        aria-label="Filter results"
      />
      <button type="button" onclick={() => scan.toggleAll()} disabled={scan.openable.length === 0}>
        {chosen.length === scan.openable.length ? "Select None" : "Select All"}
      </button>
    </div>
  {/if}

  <div class="results">
    {#if scan.error}
      <p class="message error">{scan.error.message}</p>
    {:else if scan.hits.length === 0}
      <div class="empty">
        <p class="message">
          {scan.busy
            ? "Scanning…"
            : scan.done
              ? "No repository under that folder."
              : "Pick a folder and Cogit will look through it for repositories."}
        </p>
        {#if !scan.folder && !scan.busy}
          <button type="button" onclick={onbrowse}>Choose Folder…</button>
        {/if}
      </div>
    {:else}
      {#each shown as hit (hit.root)}
        <label class="row" class:disabled={hit.alreadyOpen}>
          <input
            type="checkbox"
            checked={scan.chosen.has(hit.root)}
            disabled={hit.alreadyOpen}
            onchange={() => scan.toggle(hit.root)}
          />
          <span class="name">{hit.name}</span>
          {#if hit.bare}<span class="tag">bare</span>{/if}
          {#if hit.alreadyOpen}<span class="tag">already open</span>{/if}
          <span class="path truncate" title={hit.root}>{hit.root}</span>
        </label>
      {/each}
    {/if}
  </div>

  {#snippet footer()}
    <span class="count">
      {scan.busy ? `${scan.hits.length} found so far…` : `${scan.hits.length} found`}
    </span>
    <button type="button" onclick={onclose}>Cancel</button>
    <button type="button" class="primary" disabled={chosen.length === 0 || busy} onclick={open}>
      {openLabel}
    </button>
  {/snippet}
</Dialog>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-4) var(--sp-5);
    border-bottom: 1px solid var(--divider);
  }

  .bar input[type="search"] {
    flex: 1 1 auto;
    min-width: 0;
  }

  .folder {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .results {
    min-height: 160px;
  }

  /* Not an empty frame: a folder has to be chosen and the dialog says so and offers it. */
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--sp-4);
    min-height: 160px;
    text-align: center;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: var(--h-row-dense);
    padding: 0 var(--sp-5);
    font-size: var(--fs-dense);
    white-space: nowrap;
  }

  .row:hover {
    background: var(--state-hover);
  }

  .row.disabled {
    color: var(--text-secondary);
  }

  .name {
    flex: 0 0 auto;
  }

  .tag {
    flex: 0 0 auto;
    padding: 0 var(--sp-2);
    border-radius: var(--r-sm);
    background: var(--state-selected);
    color: var(--text-secondary);
    font-size: 10px;
  }

  .path {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .message {
    margin: 0;
    padding: var(--sp-6) var(--sp-5);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .error {
    color: var(--status-delete);
    user-select: text;
  }

  .count {
    flex: 1 1 auto;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }
</style>
