<script lang="ts">
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

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    }
  }
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose}></div>

<div class="dialog" role="dialog" aria-label="Scan folder for repositories">
  <header>
    <h2>Scan folder for repositories</h2>
    <button type="button" class="icon" onclick={onclose} aria-label="Close">✕</button>
  </header>

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
      <p class="message">
        {scan.busy ? "Scanning…" : scan.done ? "No repository under that folder." : "Choose a folder to scan."}
      </p>
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

  <footer>
    <span class="count">
      {scan.busy ? `${scan.hits.length} found so far…` : `${scan.hits.length} found`}
    </span>
    <button type="button" onclick={onclose}>Cancel</button>
    <button
      type="button"
      class="primary"
      disabled={chosen.length === 0 || busy}
      onclick={() => onopen(chosen)}
    >
      {busy ? "Opening…" : `Open ${chosen.length} Selected`}
    </button>
  </footer>
</div>

<style>
  .backdrop {
    position: absolute;
    inset: 0;
    z-index: 20;
    background: var(--scrim);
  }

  .dialog {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 21;
    display: flex;
    flex-direction: column;
    width: min(720px, 92vw);
    max-height: 82vh;
    background: var(--surface-panel);
    border: 1px solid var(--field-border);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-popover);
    overflow: hidden;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: var(--h-toolbar);
    padding: 0 var(--sp-5);
    border-bottom: 1px solid var(--divider);
  }

  h2 {
    margin: 0;
    font-size: var(--fs-ui);
    font-weight: 600;
  }

  .icon {
    background: none;
    border: 0;
    color: var(--text-secondary);
    font: inherit;
    cursor: default;
  }

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
    flex: 1 1 auto;
    min-height: 120px;
    overflow: auto;
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

  footer {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-4) var(--sp-5);
    border-top: 1px solid var(--divider);
  }

  .count {
    flex: 1 1 auto;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .primary {
    border-color: var(--status-ref);
  }
</style>
