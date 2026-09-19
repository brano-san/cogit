<script lang="ts">
  import { fileName, matchesMask, sortFiles, statusBadge, statusLabel } from "$lib/files";
  import type { SortKey } from "$lib/files";
  import { GRAPH, visibleRange } from "$lib/graph-geometry";
  import type { FileEntry } from "$lib/ipc";

  interface Props {
    files: readonly FileEntry[];
    selected?: string | null;
    onselect?: (path: string) => void;
  }

  let { files, selected = null, onselect }: Props = $props();

  const BUFFER_ROWS = 10;

  let mask = $state("");
  let sort = $state<SortKey>("path");
  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewportHeight = $state(0);

  const shown = $derived(sortFiles(files.filter((f) => matchesMask(f.path, mask)), sort));
  const range = $derived(
    visibleRange(scrollTop, viewportHeight, GRAPH.rowHeight, shown.length, BUFFER_ROWS),
  );
  const visible = $derived(
    shown.slice(range.start, range.end).map((file, index) => ({ file, row: range.start + index })),
  );

  function directory(path: string): string {
    const cut = path.lastIndexOf("/");
    return cut === -1 ? "" : path.slice(0, cut + 1);
  }

  $effect(() => {
    if (!scroller) return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry) viewportHeight = entry.contentRect.height;
    });
    observer.observe(scroller);
    return () => observer.disconnect();
  });
</script>

<div class="file-list">
  <div class="controls">
    <input
      class="mask"
      type="search"
      bind:value={mask}
      placeholder="Filter, e.g. *.rs"
      aria-label="Filter files by mask"
    />
    <select class="sort" bind:value={sort} aria-label="Sort files">
      <option value="path">Path</option>
      <option value="name">Name</option>
      <option value="status">Status</option>
    </select>
  </div>

  {#if files.length === 0}
    <p class="message">Select a commit to see the files it changed.</p>
  {:else if shown.length === 0}
    <p class="message">No file matches “{mask}”.</p>
  {:else}
    <div
      class="scroll"
      bind:this={scroller}
      onscroll={() => scroller && (scrollTop = scroller.scrollTop)}
    >
      <div class="rows" style:height="{shown.length * GRAPH.rowHeight}px">
        {#each visible as item (item.file.path)}
          <button
            type="button"
            class="row {item.file.status}"
            class:selected={selected === item.file.path}
            style:top="{item.row * GRAPH.rowHeight}px"
            title={item.file.oldPath
              ? `${item.file.oldPath} → ${item.file.path}`
              : item.file.path}
            onclick={() => onselect?.(item.file.path)}
          >
            <span class="badge" aria-label={statusLabel(item.file.status)}
              >{statusBadge(item.file.status)}</span
            >
            <span class="name truncate">{fileName(item.file.path)}</span>
            <span class="dir truncate">{directory(item.file.path)}</span>
          </button>
        {/each}
      </div>
    </div>
  {/if}
</div>

<style>
  .file-list {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .controls {
    display: flex;
    gap: var(--sp-3);
    flex: 0 0 auto;
    padding: var(--sp-3) var(--sp-4);
    border-bottom: 1px solid var(--divider);
  }

  .mask {
    flex: 1 1 auto;
    min-width: 0;
  }

  .mask,
  .sort {
    height: 20px;
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
  }

  .scroll {
    position: relative;
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .rows {
    position: relative;
  }

  .row {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: 22px;
    padding: 0 var(--sp-5);
    background: none;
    border: 0;
    color: inherit;
    font: inherit;
    font-size: var(--fs-dense);
    text-align: left;
    white-space: nowrap;
    cursor: default;
  }

  .row:hover {
    background: var(--state-hover);
  }

  .row.selected {
    background: var(--state-selected);
  }

  .badge {
    flex: 0 0 auto;
    width: 12px;
    font-family: var(--font-mono);
    font-weight: 600;
    text-align: center;
  }

  .row.added .badge {
    color: var(--status-add);
  }

  .row.modified .badge {
    color: var(--status-modify);
  }

  .row.deleted .badge {
    color: var(--status-delete);
  }

  .row.renamed .badge,
  .row.copied .badge {
    color: var(--status-ref);
  }

  .name {
    flex: 0 1 auto;
    min-width: 0;
  }

  .dir {
    flex: 1 1 auto;
    min-width: 0;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .message {
    margin: 0;
    padding: var(--sp-5);
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }
</style>
