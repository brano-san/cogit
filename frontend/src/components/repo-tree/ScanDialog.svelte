<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import Disclosure from "$components/common/Disclosure.svelte";
  import { scan } from "$stores/scan.svelte";
  import { scanSections, type ScanLayout } from "$lib/scan-groups";
  import type { ScanHit } from "$lib/ipc";

  interface Props {
    busy: boolean;
    onbrowse: () => void;
    onopen: (roots: string[]) => void;
    onclose: () => void;
  }

  let { busy, onbrowse, onopen, onclose }: Props = $props();

  let filter = $state("");
  let layout = $state<ScanLayout>("folders");
  /** Groups the user folded, by title; every group starts open, in both layouts. */
  let folded = $state.raw<ReadonlySet<string>>(new Set());

  function fold(title: string) {
    const next = new Set(folded);
    if (!next.delete(title)) next.add(title);
    folded = next;
  }

  const shown = $derived(
    scan.hits.filter(
      (hit) =>
        filter.trim() === "" ||
        `${hit.name} ${hit.root}`.toLowerCase().includes(filter.trim().toLowerCase()),
    ),
  );
  const sections = $derived(scanSections(shown, layout, scan.folder ?? ""));
  const chosen = $derived(scan.selected);
  const shownOpenable = $derived(shown.filter((hit) => !hit.alreadyOpen));

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

{#snippet row(hit: ScanHit)}
  <div class="row" class:disabled={hit.alreadyOpen}>
    <Checkbox wide checked={scan.chosen.has(hit.root)} disabled={hit.alreadyOpen} onchange={() => scan.toggle(hit.root)}>
      <span class="name">{hit.name}</span>
      <span class="path">{hit.root}</span>
    </Checkbox>
  </div>
{/snippet}

<Dialog
  title="Search for repositories"
  width="min(720px, 92vw)"
  flush
  {onclose}
  onconfirm={open}
>
  <div class="bar">
    <button class="btn" type="button" onclick={onbrowse}>Choose Folder…</button>
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
      <button
        class="btn"
        type="button"
        onclick={() => scan.toggleAll(shown.map((hit) => hit.root))}
        disabled={shownOpenable.length === 0}
      >
        {shownOpenable.every((hit) => scan.chosen.has(hit.root)) ? "Select None" : "Select All"}
      </button>
      <button class="btn" type="button" onclick={() => (layout = layout === "folders" ? "flat" : "folders")}>
        {layout === "folders" ? "Flat List" : "Group by Folder"}
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
              ? "No repository found."
              : "Pick a folder and Cogit will look through it for repositories."}
        </p>
        {#if !scan.folder && !scan.busy}
          <button class="btn" type="button" onclick={onbrowse}>Choose Folder…</button>
        {/if}
      </div>
    {:else}
      <!-- As wide as the longest path: the list scrolls sideways instead of cutting it. -->
      <div class="rows">
        {#each sections as section (section.title)}
          {#if section.title === null}
            {#each section.hits as hit (hit.root)}{@render row(hit)}{/each}
          {:else}
            {@const title = section.title}
            {@const open = !folded.has(title)}
            <div class="group">
              <button type="button" class="heading" aria-expanded={open} onclick={() => fold(title)}>
                <Disclosure {open} />
                <span>{title}</span>
                <span class="n">{section.hits.length}</span>
              </button>
              {#if open}
                {#each section.hits as hit (hit.root)}{@render row(hit)}{/each}
              {/if}
            </div>
          {/if}
        {/each}
      </div>
    {/if}
  </div>

  {#snippet footer()}
    <span class="count">
      {scan.busy ? `${scan.hits.length} found so far…` : `${scan.hits.length} found`}
    </span>
    <button class="btn" type="button" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" disabled={chosen.length === 0 || busy} onclick={open}>
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
    flex: 1 1 auto;
    min-height: 160px;
    overflow: auto;
    overflow-wrap: normal;
  }

  .rows {
    width: max-content;
    min-width: 100%;
  }

  /* A group is a block: a frame around its heading and its rows, apart from the next one. */
  .group {
    margin: var(--sp-3) var(--sp-5);
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
    overflow: hidden;
  }

  .heading {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    width: 100%;
    height: var(--h-row-dense);
    padding: 0 var(--disclosure-side);
    background: var(--surface-base);
    color: var(--text-secondary);
    border: 0;
    font: inherit;
    font-size: var(--fs-header);
    font-weight: 600;
    text-align: left;
    white-space: nowrap;
    cursor: default;
  }

  .heading:hover {
    color: var(--text-primary);
  }

  .group:has(.row) .heading {
    border-bottom: 1px solid var(--divider);
  }

  .heading .n {
    font-weight: 400;
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

  .group .row {
    padding-left: calc(var(--disclosure-side) * 2 + var(--disclosure-glyph));
  }

  .row.disabled {
    color: var(--text-secondary);
  }

  .name {
    flex: 0 0 auto;
  }

  .path {
    flex: 0 0 auto;
    color: var(--text-secondary);
    font-size: var(--fs-header);
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
