<script lang="ts">
  import { fileName, matchesMask, sortFiles, statusBadge, statusLabel } from "$lib/files";
  import type { SortKey } from "$lib/files";
  import { GRAPH, visibleRange } from "$lib/graph-geometry";
  import type { FileEntry } from "$lib/ipc";

  interface Action {
    label: string;
    title: string;
    run: (paths: string[]) => void;
  }

  interface Section {
    title?: string;
    files: readonly FileEntry[];
    actions?: readonly Action[];
    /** Each section can open its own side of the diff. */
    onselect?: (path: string) => void;
  }

  interface Props {
    sections: readonly Section[];
    selected?: string | null;
    empty?: string;
    onselect?: (path: string) => void;
    /** Reported upward so Commit What You See knows what is hidden (T6.8). */
    onmask?: (mask: string) => void;
  }

  let { sections, selected = null, empty, onselect, onmask }: Props = $props();

  const BUFFER_ROWS = 10;

  let mask = $state("");

  $effect(() => {
    onmask?.(mask);
  });
  let sort = $state<SortKey>("path");
  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewportHeight = $state(0);

  type Row =
    | { kind: "header"; title: string; paths: string[]; actions: readonly Action[] }
    | {
        kind: "file";
        file: FileEntry;
        actions: readonly Action[];
        open?: (path: string) => void;
      };

  const total = $derived(sections.reduce((n, s) => n + s.files.length, 0));
  const shown = $derived.by(() => {
    const rows: Row[] = [];
    for (const section of sections) {
      const kept = sortFiles(
        section.files.filter((f) => matchesMask(f.path, mask)),
        sort,
      );
      if (kept.length === 0) continue;
      const actions = section.actions ?? [];
      if (section.title) {
        rows.push({
          kind: "header",
          title: `${section.title} (${kept.length})`,
          paths: kept.map((f) => f.path),
          actions,
        });
      }
      for (const file of kept) {
        rows.push({ kind: "file", file, actions, open: section.onselect });
      }
    }
    return rows;
  });
  const range = $derived(
    visibleRange(scrollTop, viewportHeight, GRAPH.rowHeight, shown.length, BUFFER_ROWS),
  );
  const visible = $derived(
    shown.slice(range.start, range.end).map((row, index) => ({ row, at: range.start + index })),
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

  {#if total === 0}
    <p class="message">{empty ?? "Nothing to show."}</p>
  {:else if shown.length === 0}
    <p class="message">No file matches “{mask}”.</p>
  {:else}
    <div
      class="scroll"
      bind:this={scroller}
      onscroll={() => scroller && (scrollTop = scroller.scrollTop)}
    >
      <div class="rows" style:height="{shown.length * GRAPH.rowHeight}px">
        {#each visible as item (item.at)}
          {#if item.row.kind === "header"}
            {@const header = item.row}
            <div class="section" style:top="{item.at * GRAPH.rowHeight}px">
              <span class="grow">{header.title}</span>
              {#each header.actions as action (action.label)}
                <button
                  type="button"
                  class="act"
                  title="{action.title} — all"
                  onclick={() => action.run(header.paths)}>{action.label} all</button
                >
              {/each}
            </div>
          {:else}
            {@const file = item.row.file}
            <button
              type="button"
              class="row {file.status}"
              class:selected={selected === file.path}
              style:top="{item.at * GRAPH.rowHeight}px"
              title={file.oldPath ? `${file.oldPath} → ${file.path}` : file.path}
              onclick={() => (item.row.kind === "file" ? (item.row.open ?? onselect)?.(file.path) : undefined)}
            >
              <span class="badge" aria-label={statusLabel(file.status)}>{statusBadge(file.status)}</span>
              <span class="name truncate">{fileName(file.path)}</span>
              {#if file.oldPath}
                <span class="renamed truncate" title="from {file.oldPath}"
                  >← {fileName(file.oldPath)}{file.similarity !== null
                    ? ` ${file.similarity}%`
                    : ""}</span
                >
              {/if}
              {#if file.modeChange}
                <span class="mode" title="Mode changed to {file.modeChange}"
                  >{file.modeChange === "executable" ? "+x" : "−x"}</span
                >
              {/if}
              <span class="dir truncate">{directory(file.path)}</span>
              {#each item.row.actions as action (action.label)}
                <span
                  class="act"
                  role="button"
                  tabindex="-1"
                  title={action.title}
                  onclick={(event) => {
                    event.stopPropagation();
                    action.run([file.path]);
                  }}
                  onkeydown={(event) => {
                    if (event.key === "Enter") action.run([file.path]);
                  }}>{action.label}</span
                >
              {/each}
            </button>
          {/if}
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

  .grow {
    flex: 1 1 auto;
  }

  .act {
    flex: 0 0 auto;
    padding: 0 var(--sp-3);
    background: none;
    border: 0;
    color: var(--text-secondary);
    font: inherit;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0;
    cursor: default;
  }

  .row:hover .act,
  .section:hover .act {
    opacity: 1;
  }

  .act:hover {
    color: var(--status-ref);
  }

  .section {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    height: 22px;
    padding: 0 var(--sp-5);
    background: var(--surface-raised);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
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

  .row.untracked .badge {
    color: var(--text-secondary);
  }

  .row.conflicted .badge {
    color: var(--status-delete);
    background: var(--c-deleted-bg, rgb(90 40 40 / 45%));
  }

  .name {
    flex: 0 1 auto;
    min-width: 0;
  }

  .renamed {
    flex: 0 1 auto;
    min-width: 0;
    color: var(--status-ref);
    font-size: 10px;
  }

  .mode {
    flex: 0 0 auto;
    color: var(--status-modify);
    font-family: var(--font-mono);
    font-size: 10px;
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
