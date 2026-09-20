<script lang="ts">
  import { fileName, statusBadge, statusLabel } from "$lib/files";
  import type { ViewRow } from "$lib/file-view";
  import { GRAPH, visibleRange } from "$lib/graph-geometry";

  interface Action {
    label: string;
    title: string;
    run: (paths: string[]) => void;
  }

  interface Props {
    rows: readonly ViewRow[];
    title?: string;
    selected?: string | null;
    marked: ReadonlySet<string>;
    actions?: readonly Action[];
    /** Paths shown here, for the "all" buttons in the heading. */
    paths: readonly string[];
    /** Full paths are redundant once the list groups by directory. */
    showDirectory?: boolean;
    onclick: (path: string, event: MouseEvent) => void;
    onmark: (path: string) => void;
    onopen?: (path: string) => void;
  }

  let {
    rows,
    title,
    selected = null,
    marked,
    actions = [],
    paths,
    showDirectory = true,
    onclick,
    onmark,
    onopen,
  }: Props = $props();

  const BUFFER_ROWS = 10;

  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewportHeight = $state(0);

  const range = $derived(
    visibleRange(scrollTop, viewportHeight, GRAPH.rowHeight, rows.length, BUFFER_ROWS),
  );
  const visible = $derived(
    rows.slice(range.start, range.end).map((row, index) => ({ row, at: range.start + index })),
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

<div class="pane">
  {#if title}
    <div class="heading">
      <span class="grow">{title} ({paths.length})</span>
      {#each actions as action (action.label)}
        <button type="button" class="act" title="{action.title} — all" onclick={() => action.run([...paths])}
          >{action.label} all</button
        >
      {/each}
    </div>
  {/if}

  <div
    class="scroll"
    bind:this={scroller}
    onscroll={() => scroller && (scrollTop = scroller.scrollTop)}
  >
    <div class="rows" style:height="{rows.length * GRAPH.rowHeight}px">
      {#each visible as item (item.at)}
        {#if item.row.kind === "dir"}
          {@const group = item.row}
          <div class="folder" style:top="{item.at * GRAPH.rowHeight}px">
            <span aria-hidden="true">▾</span>
            <span class="truncate">{group.path === "" ? "(root)" : group.path}</span>
            <span class="count">{group.count}</span>
          </div>
        {:else}
          {@const file = item.row.file}
          <button
            type="button"
            class="row {file.status}"
            class:selected={selected === file.path}
            class:marked={marked.has(file.path)}
            style:top="{item.at * GRAPH.rowHeight}px"
            title={file.oldPath ? `${file.oldPath} → ${file.path}` : file.path}
            onclick={(event) => onclick(file.path, event)}
            onkeydown={(event) => {
              if (event.key !== " ") return;
              event.preventDefault();
              onmark(file.path);
            }}
            ondblclick={() => onopen?.(file.path)}
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
            <span class="dir truncate">{showDirectory ? directory(file.path) : ""}</span>
            {#each actions as action (action.label)}
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
</div>

<style>
  .pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }

  .heading {
    display: flex;
    align-items: center;
    flex: 0 0 auto;
    height: var(--h-row-dense);
    padding: 0 var(--sp-5);
    background: var(--surface-raised);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
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

  .row,
  .folder {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: 22px;
    padding: 0 var(--sp-5);
    font-size: var(--fs-dense);
    white-space: nowrap;
  }

  .row {
    background: none;
    border: 0;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: default;
  }

  .row:hover {
    background: var(--state-hover);
  }

  .row.marked {
    box-shadow: inset 2px 0 0 var(--status-ref);
  }

  .row.selected {
    background: var(--state-selected);
  }

  .folder {
    color: var(--text-secondary);
    font-size: 11px;
  }

  .folder .count {
    margin-left: auto;
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
  .heading:hover .act {
    opacity: 1;
  }

  .act:hover {
    color: var(--status-ref);
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

  .row.untracked .badge,
  .row.unchanged .badge,
  .row.ignored .badge,
  .row.assumeUnchanged .badge,
  .row.skipped .badge {
    color: var(--text-secondary);
  }

  .row.unchanged,
  .row.ignored,
  .row.assumeUnchanged,
  .row.skipped {
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
</style>
